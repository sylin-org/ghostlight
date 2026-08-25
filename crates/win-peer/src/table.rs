//! `GetExtendedTcpTable` walk that resolves a connection quadruple to its owning process.
//!
//! ADR-0105 stage 2. The kernel's answer for who owns a connection is not forgeable by the
//! caller, unlike every field of the hello it sent.

use std::net::{IpAddr, SocketAddr, TcpStream};

const AF_INET: u16 = 2;
/// `TCP_TABLE_OWNER_PID_CONNECTIONS` from `iphlpapi.h`.
const TCP_TABLE_OWNER_PID_CONNECTIONS: u32 = 4;
/// `ERROR_INSUFFICIENT_BUFFER`.
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

#[repr(C)]
struct MibTcpRowOwnerPid {
    dw_state: u32,
    dw_local_addr: u32,
    dw_local_port: u32,
    dw_remote_addr: u32,
    dw_remote_port: u32,
    dw_owning_pid: u32,
}

#[link(name = "iphlpapi")]
extern "system" {
    fn GetExtendedTcpTable(
        tcp_table: *mut core::ffi::c_void,
        size_pointer: *mut u32,
        order: i32,
        address_family: u16,
        table_class: u32,
        reserved: u32,
    ) -> u32;
}

/// Resolve the owning process of the peer end of one accepted loopback connection.
pub(super) fn identify(stream: &TcpStream) -> Option<super::PeerIdentity> {
    let local = stream.local_addr().ok()?;
    let peer = stream.peer_addr().ok()?;
    identify_addresses(local, peer)
}

/// Resolve the owning process from an already-captured connection quadruple.
pub(super) fn identify_addresses(
    local: SocketAddr,
    peer: SocketAddr,
) -> Option<super::PeerIdentity> {
    let row = find_row(local, peer)?;
    let image_path = super::image::full_image_path(row.dw_owning_pid)?;
    let image_name = super::image::bounded_name(&image_path)?;
    Some(super::PeerIdentity {
        process_id: row.dw_owning_pid,
        image_name,
    })
}

fn find_row(local: SocketAddr, peer: SocketAddr) -> Option<MibTcpRowOwnerPid> {
    let expected_local_addr = match local.ip() {
        IpAddr::V4(ipv4) => u32::from_be_bytes(ipv4.octets()),
        IpAddr::V6(_) => return None,
    };
    let expected_remote_addr = match peer.ip() {
        IpAddr::V4(ipv4) => u32::from_be_bytes(ipv4.octets()),
        IpAddr::V6(_) => return None,
    };
    let expected_local_port = local.port();
    let expected_remote_port = peer.port();
    let mut size = 0u32;
    // SAFETY: the null buffer probe only writes the required size through size_pointer.
    let code = unsafe {
        GetExtendedTcpTable(
            std::ptr::null_mut(),
            &mut size,
            0,
            AF_INET,
            TCP_TABLE_OWNER_PID_CONNECTIONS,
            0,
        )
    };
    if code != ERROR_INSUFFICIENT_BUFFER || size == 0 {
        return None;
    }
    // The table can grow between the size probe and the read; retry a bounded number of times
    // with a grown buffer before giving up.
    let mut buffer = Vec::new();
    let mut code = 0u32;
    for _ in 0..3 {
        buffer = vec![0u8; size as usize];
        // SAFETY: buffer is sized by the preceding probe and the table class matches the row
        // layout this module parses (a u32 entry count followed by packed 24-byte rows).
        code = unsafe {
            GetExtendedTcpTable(
                buffer.as_mut_ptr().cast(),
                &mut size,
                0,
                AF_INET,
                TCP_TABLE_OWNER_PID_CONNECTIONS,
                0,
            )
        };
        if code == 0 {
            break;
        }
        if code != ERROR_INSUFFICIENT_BUFFER {
            return None;
        }
        size = size.saturating_mul(2);
    }
    if code != 0 {
        return None;
    }
    read_rows(&buffer).into_iter().find(|row| {
        // Addresses arrive in network byte order inside their dwords; ports already go through
        // port_of. Normalize both sides before comparing.
        u32::from_be(row.dw_local_addr) == expected_local_addr
            && u32::from_be(row.dw_remote_addr) == expected_remote_addr
            && port_of(row.dw_local_port) == expected_local_port
            && port_of(row.dw_remote_port) == expected_remote_port
    })
}

fn read_rows(buffer: &[u8]) -> Vec<MibTcpRowOwnerPid> {
    const ROW_SIZE: usize = std::mem::size_of::<MibTcpRowOwnerPid>();
    if buffer.len() < 4 {
        return Vec::new();
    }
    let count = u32::from_ne_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
    let available = (buffer.len() - 4) / ROW_SIZE;
    (0..count.min(available))
        .map(|index| {
            let start = 4 + index * ROW_SIZE;
            let mut row_bytes = [0u8; ROW_SIZE];
            row_bytes.copy_from_slice(&buffer[start..start + ROW_SIZE]);
            // SAFETY: ROW_SIZE bytes were copied into an array whose alignment matches the
            // repr(C) row layout, so the transmute reads exactly the fields written above.
            unsafe { std::mem::transmute::<[u8; ROW_SIZE], MibTcpRowOwnerPid>(row_bytes) }
        })
        .collect()
}

fn port_of(raw: u32) -> u16 {
    u16::from_be((raw & 0xFFFF) as u16)
}

/// A plain copy of one parsed row, for tests and diagnosis.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RowCopy {
    pub state: u32,
    pub local_addr: u32,
    pub local_port: u16,
    pub remote_addr: u32,
    pub remote_port: u16,
    pub owning_pid: u32,
}

/// Every parsed row of the current connection table.
#[cfg(test)]
pub(super) fn rows() -> Vec<RowCopy> {
    let mut size = 0u32;
    // SAFETY: null-buffer probe writes the required size only.
    let code = unsafe {
        GetExtendedTcpTable(
            std::ptr::null_mut(),
            &mut size,
            0,
            AF_INET,
            TCP_TABLE_OWNER_PID_CONNECTIONS,
            0,
        )
    };
    if code != ERROR_INSUFFICIENT_BUFFER || size == 0 {
        return Vec::new();
    }
    let mut buffer = vec![0u8; size as usize];
    // SAFETY: as in find_row: buffer sized by the probe, row layout matches the table class.
    let code = unsafe {
        GetExtendedTcpTable(
            buffer.as_mut_ptr().cast(),
            &mut size,
            0,
            AF_INET,
            TCP_TABLE_OWNER_PID_CONNECTIONS,
            0,
        )
    };
    if code != 0 {
        return Vec::new();
    }
    read_rows(&buffer)
        .into_iter()
        .map(|row| RowCopy {
            state: row.dw_state,
            local_addr: row.dw_local_addr,
            local_port: port_of(row.dw_local_port),
            remote_addr: row.dw_remote_addr,
            remote_port: port_of(row.dw_remote_port),
            owning_pid: row.dw_owning_pid,
        })
        .collect()
}
