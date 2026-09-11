//! Observed socket-peer identity for local connections (ADR-0105 stage 2).
//!
//! This is the one audited FFI crate in the workspace. The workspace sets
//! `unsafe_code = "forbid"` through the shared lints table, which Cargo applies as command-line
//! flags no in-source allow can override; this crate's manifest deliberately does not inherit
//! that table, so every line of `unsafe` in Ghostlight lives here, is reviewed as a unit, and a
//! repository guard test fails if `unsafe` appears anywhere else. The owner decision recorded in
//! ADR-0105 chose this shape over wrapping the same calls behind a new third-party dependency on
//! a security-sensitive path: every foreign function is declared by hand below against system
//! link libraries, with a `// SAFETY:` note at each call site.
//!
//! Socket-peer observation: [`identify_connection`] resolves the owning process of the remote
//! endpoint of a loopback connection through `GetExtendedTcpTable`, and [`PeerIdentity`] carries its
//! id with the executable's bounded lowercase file name. The name only -- never the path -- is
//! what may reach audit or presentation surfaces (ADR-0105 Decision 2).
//!
//! Signer-gated admission (ADR-0105 stage 3) stays deferred and this crate deliberately contains
//! no signature-verification code. The September 6 amendment requires a concrete verifiable
//! integration and real signed-subject evidence, without requiring Ghostlight's own certificate.
//! Non-Windows targets compile the same surface returning `None`. ADR-0160 adds Windows-only
//! creation of owner-private runtime files in `private_file`, keeping creation-time security
//! attributes inside this same FFI boundary.

/// The process observed to own the remote endpoint of a local connection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerIdentity {
    /// The operating-system process id that owns the socket.
    pub process_id: u32,
    /// Bounded lowercase file name of the peer executable. Never the path.
    pub image_name: String,
}

/// Stable identity for one Windows process across process-id reuse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessIdentity {
    /// Operating-system process id.
    pub process_id: u32,
    /// Windows process creation time, expressed as the raw FILETIME value.
    pub created: u64,
}

#[cfg(target_os = "windows")]
mod image;
#[cfg(target_os = "windows")]
mod private_file;
#[cfg(target_os = "windows")]
mod process;
#[cfg(target_os = "windows")]
mod table;

/// Capture the current process's parent with creation time bound to its process id.
#[must_use]
pub fn parent_process() -> Option<ProcessIdentity> {
    #[cfg(target_os = "windows")]
    return process::parent();
    #[cfg(not(target_os = "windows"))]
    None
}

/// Capture one process with creation time bound to its process id.
#[must_use]
pub fn process_identity(process_id: u32) -> Option<ProcessIdentity> {
    #[cfg(target_os = "windows")]
    return process::identity(process_id);
    #[cfg(not(target_os = "windows"))]
    {
        let _ = process_id;
        None
    }
}

/// Return whether the exact captured Windows process is still running.
#[must_use]
pub fn process_is_alive(process: ProcessIdentity) -> bool {
    #[cfg(target_os = "windows")]
    return process::is_alive(process);
    #[cfg(not(target_os = "windows"))]
    {
        let _ = process;
        false
    }
}

/// Create a new file with a protected current-user/SYSTEM DACL before it can contain secrets.
/// Existing paths are refused. Runtime publication belongs to the caller (ADR-0160).
#[cfg(target_os = "windows")]
pub fn create_private_file(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    private_file::create(path)
}

/// Identify the process that owns the remote endpoint of a local connection.
///
/// Returns `None` where the platform cannot answer, the socket is gone before the table walk
/// finishes, or the peer does not appear in the connection table.
#[must_use]
pub fn identify_connection(stream: &std::net::TcpStream) -> Option<PeerIdentity> {
    #[cfg(target_os = "windows")]
    return table::identify(stream);
    #[cfg(not(target_os = "windows"))]
    {
        let _ = stream;
        None
    }
}

/// Identify the remote endpoint's owning process from the observer's connection addresses.
///
/// A caller whose socket has already been moved into another structure can capture
/// [`std::net::TcpStream::local_addr`] and [`std::net::TcpStream::peer_addr`] first and call this
/// instead of [`identify_connection`]. Pass the observer's local address first and its peer
/// address second; the returned identity belongs to the peer, never the observer's local row.
#[must_use]
pub fn identify_addresses(
    local: std::net::SocketAddr,
    peer: std::net::SocketAddr,
) -> Option<PeerIdentity> {
    #[cfg(target_os = "windows")]
    return table::identify_addresses(local, peer);
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (local, peer);
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(target_os = "windows")]
    fn a_live_loopback_pair_identifies_its_own_process() {
        use std::io::Write as _;
        use std::net::{TcpListener, TcpStream};

        use crate::identify_connection;

        // The table walk must resolve an exact connection quadruple to its owning process id.
        // An in-process pair checks lookup mechanics but cannot prove which endpoint was read.
        // tests/provenance-journey.mjs proves direction with distinct executable processes.
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let mut stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        stream.write_all(&[1]).unwrap();

        // On failure, name every row near our local port so the diagnosis is in the panic text.
        let identity = identify_connection(&stream).unwrap_or_else(|| {
            let local = stream.local_addr().unwrap().port();
            let near: Vec<_> = crate::table::rows()
                .into_iter()
                .filter(|row| [row.local_port, row.remote_port].contains(&local))
                .collect();
            panic!("no row for quadruple {local:?}; nearby rows: {near:?}");
        });
        assert_eq!(identity.process_id, std::process::id());
        assert_eq!(identity.image_name.to_lowercase(), identity.image_name);
        assert!(!identity.image_name.is_empty());
        assert!(identity.image_name.len() <= 120, "names stay bounded");
        assert!(
            !identity.image_name.contains('\\'),
            "name only, never a path"
        );
    }

    /// Cross-platform negative control: no such connection exists, so no platform may invent
    /// an owner for it. This also keeps the stubbed non-Windows surface compiled and pinned.
    #[test]
    fn an_absent_quadruple_identifies_nothing() {
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};
        let local = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1);
        let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 2);
        assert!(super::identify_addresses(local, peer).is_none());
    }
}
