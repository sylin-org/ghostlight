// SPDX-License-Identifier: Apache-2.0 OR MIT
//! ADR-0096: the bare `ghostlight` (no subcommand) does not serve MCP. It points the user at the
//! dedicated `ghostlight-mcp-connector` protocol edge and exits 2.

use std::io::Read;
use std::process::{Command, Stdio};

#[test]
fn bare_invocation_prints_guidance_and_exits_2() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ghostlight"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn bare ghostlight");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("child stderr")
        .read_to_string(&mut stderr)
        .expect("read stderr");
    let status = child.wait().expect("wait for bare ghostlight");
    assert_eq!(
        status.code(),
        Some(2),
        "bare invocation exits 2; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("your MCP client launches ghostlight-mcp-connector"),
        "stderr carries the ADR-0096 guidance:\n{stderr}"
    );
}
