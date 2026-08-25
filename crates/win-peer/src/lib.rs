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
//! One capability is exposed: [`identify_connection`] resolves the owning process of an accepted
//! loopback connection through `GetExtendedTcpTable`, and [`PeerIdentity`] carries that process
//! id with the executable's bounded lowercase file name. The name only -- never the path -- is
//! what may reach audit or presentation surfaces (ADR-0105 Decision 2).
//!
//! Signer-gated admission (ADR-0105 stage 3) stays deferred and this crate deliberately contains
//! no signature-verification code: revisit it when Ghostlight's first signed artifact exists to
//! verify against. Non-Windows targets compile the same surface returning `None`, so callers
//! stay branch-free.

/// The process observed to own one side of a local connection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeerIdentity {
    /// The operating-system process id that owns the socket.
    pub process_id: u32,
    /// Bounded lowercase file name of the peer executable. Never the path.
    pub image_name: String,
}

#[cfg(target_os = "windows")]
mod image;
#[cfg(target_os = "windows")]
mod table;

/// Identify the process that owns the peer end of an accepted local connection.
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

/// Identify the owning process from one connection's address quadruple directly.
///
/// A caller whose socket has already been moved into another structure can capture
/// [`std::net::TcpStream::local_addr`] and [`std::net::TcpStream::peer_addr`] first and call this
/// instead of [`identify_connection`].
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
    use super::*;

    #[test]
    #[cfg(target_os = "windows")]
    fn a_live_loopback_pair_identifies_its_own_process() {
        use std::io::Write as _;
        use std::net::{TcpListener, TcpStream};

        // The table walk must resolve an exact connection quadruple to its owning process id.
        // An in-process loopback pair pins the mechanics deterministically; the cross-process
        // case is what the CLI journey proves through real connector processes.
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
}
