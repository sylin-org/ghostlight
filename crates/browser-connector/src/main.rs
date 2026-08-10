//! Opaque, reconnecting native-message relay for the Ghostlight browser adapter.

use std::io;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use ghostlight_bridge::framing::{
    read_length_frame, read_native, write_length_frame, write_native,
};
use ghostlight_bridge::relay::{
    BrowserRelayRequest, BrowserRelayResponse, BrowserRelayStatus, BROWSER_RELAY_MAJOR,
};
use ghostlight_bridge::runtime::{read_runtime, runtime_file};

const FRAME_BUFFER: usize = 64;
const RETRY_INTERVAL: Duration = Duration::from_millis(500);

enum RelayEvent {
    ChromeFrame(Vec<u8>),
    ChromeClosed,
    ChromeFailed(String),
    ServiceFrame { generation: u64, payload: Vec<u8> },
    ServiceClosed { generation: u64 },
}

fn main() -> Result<()> {
    let (events, incoming) = sync_channel(FRAME_BUFFER);
    let chrome_alive = Arc::new(AtomicBool::new(true));
    spawn_chrome_reader(events.clone(), Arc::clone(&chrome_alive))?;

    let adapter_hello = receive_adapter_hello(&incoming)?;
    let stdout = io::stdout();
    let mut chrome_output = stdout.lock();
    let mut service: Option<TcpStream> = None;
    let mut generation = 0_u64;

    loop {
        if service.is_none() {
            let connection = match connect_once(&adapter_hello) {
                Ok(connection) => Some(connection),
                Err(_) => {
                    write_native(&mut chrome_output, &BrowserRelayStatus::BackendUnavailable)
                        .context("report unavailable backend")?;
                    connect_adapter(&adapter_hello, &chrome_alive)?
                }
            };
            let Some((stream, first_adapter_response)) = connection else {
                return Ok(());
            };
            generation = generation.wrapping_add(1);
            write_length_frame(&mut chrome_output, &first_adapter_response)
                .context("forward adapter negotiation")?;
            spawn_service_reader(stream.try_clone()?, generation, events.clone())?;
            service = Some(stream);
        }

        match incoming.recv() {
            Ok(RelayEvent::ChromeFrame(payload)) => {
                let Some(writer) = service.as_mut() else {
                    continue;
                };
                if write_length_frame(writer, &payload).is_err() {
                    service = None;
                }
            }
            Ok(RelayEvent::ChromeClosed) | Err(_) => return Ok(()),
            Ok(RelayEvent::ChromeFailed(message)) => {
                anyhow::bail!("native input failed: {message}")
            }
            Ok(RelayEvent::ServiceFrame {
                generation: source,
                payload,
            }) if source == generation => {
                write_length_frame(&mut chrome_output, &payload)
                    .context("forward service adapter frame")?;
            }
            Ok(RelayEvent::ServiceClosed { generation: source }) if source == generation => {
                service = None;
            }
            Ok(RelayEvent::ServiceFrame { .. } | RelayEvent::ServiceClosed { .. }) => {}
        }
    }
}

fn spawn_chrome_reader(
    events: SyncSender<RelayEvent>,
    chrome_alive: Arc<AtomicBool>,
) -> Result<()> {
    thread::Builder::new()
        .name("ghostlight-native-input".into())
        .spawn(move || {
            let stdin = io::stdin();
            let mut input = stdin.lock();
            loop {
                match read_length_frame(&mut input) {
                    Ok(Some(payload)) => {
                        if events.send(RelayEvent::ChromeFrame(payload)).is_err() {
                            return;
                        }
                    }
                    Ok(None) => {
                        chrome_alive.store(false, Ordering::SeqCst);
                        let _ = events.send(RelayEvent::ChromeClosed);
                        return;
                    }
                    Err(error) => {
                        chrome_alive.store(false, Ordering::SeqCst);
                        let _ = events.send(RelayEvent::ChromeFailed(error.to_string()));
                        return;
                    }
                }
            }
        })?;
    Ok(())
}

fn receive_adapter_hello(incoming: &Receiver<RelayEvent>) -> Result<Vec<u8>> {
    match incoming.recv() {
        Ok(RelayEvent::ChromeFrame(payload)) => Ok(payload),
        Ok(RelayEvent::ChromeClosed) | Err(_) => Ok(Vec::new()),
        Ok(RelayEvent::ChromeFailed(message)) => anyhow::bail!("read adapter hello: {message}"),
        Ok(RelayEvent::ServiceFrame { .. } | RelayEvent::ServiceClosed { .. }) => {
            anyhow::bail!("service event arrived before adapter hello")
        }
    }
}

fn connect_adapter(
    adapter_hello: &[u8],
    chrome_alive: &AtomicBool,
) -> Result<Option<(TcpStream, Vec<u8>)>> {
    if adapter_hello.is_empty() {
        return Ok(None);
    }
    while chrome_alive.load(Ordering::SeqCst) {
        if let Ok(connection) = connect_once(adapter_hello) {
            return Ok(Some(connection));
        }
        thread::sleep(RETRY_INTERVAL);
    }
    Ok(None)
}

fn connect_once(adapter_hello: &[u8]) -> Result<(TcpStream, Vec<u8>)> {
    let endpoint = read_runtime(&runtime_file()).context("read current Ghostlight runtime")?;
    if endpoint.browser_relay_major != BROWSER_RELAY_MAJOR {
        anyhow::bail!(
            "browser relay major {} is incompatible with required {}",
            endpoint.browser_relay_major,
            BROWSER_RELAY_MAJOR
        );
    }
    let mut service = TcpStream::connect(("127.0.0.1", endpoint.browser_port))
        .context("connect browser relay")?;
    service.set_nodelay(true)?;
    write_native(
        &mut service,
        &BrowserRelayRequest::Hello {
            major: BROWSER_RELAY_MAJOR,
            token: endpoint.token,
        },
    )
    .context("send browser relay hello")?;
    match read_native::<BrowserRelayResponse>(&mut service).context("read browser relay hello")? {
        Some(BrowserRelayResponse::Accepted { major }) if major == BROWSER_RELAY_MAJOR => {}
        Some(BrowserRelayResponse::Rejected { code, message }) => {
            anyhow::bail!("browser relay rejected connection ({code}): {message}")
        }
        _ => anyhow::bail!("browser relay returned an invalid hello response"),
    }
    write_length_frame(&mut service, adapter_hello).context("replay opaque adapter hello")?;
    let first_adapter_response = read_length_frame(&mut service)
        .context("read adapter negotiation")?
        .context("service closed during adapter negotiation")?;
    Ok((service, first_adapter_response))
}

fn spawn_service_reader(
    mut service: TcpStream,
    generation: u64,
    events: SyncSender<RelayEvent>,
) -> Result<()> {
    thread::Builder::new()
        .name("ghostlight-native-output".into())
        .spawn(move || loop {
            match read_length_frame(&mut service) {
                Ok(Some(payload)) => {
                    if events
                        .send(RelayEvent::ServiceFrame {
                            generation,
                            payload,
                        })
                        .is_err()
                    {
                        return;
                    }
                }
                Ok(None) | Err(_) => {
                    let _ = events.send(RelayEvent::ServiceClosed { generation });
                    return;
                }
            }
        })?;
    Ok(())
}
