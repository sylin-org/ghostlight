//! The requests in a client line too long to keep (ADR-0197 amendment of 2026-10-21).
//!
//! A line over [`MAX_FRAME_BYTES`] can reach no service, so the edge answers each request in it
//! from the service's `too_large` template. The line itself is never kept: a [`Scanner`] reads it
//! a chunk at a time while the edge skips it, and keeps only what an answer needs, each request's
//! id and method. The line is one JSON-RPC message or a batch array of them, and a message's
//! members may come in any order (the TypeScript SDK writes the id after the params), so the
//! scanner follows the JSON structure: keys in any order, strings with their escapes, and nested
//! values skipped by depth.
//!
//! What it keeps is bounded. An id longer than [`ID_LIMIT`] bytes of JSON cannot be echoed, so its
//! request is left unanswered; a method longer than [`METHOD_LIMIT`] is read as no method, which
//! the `default` template answers; and a batch is read for its first [`MESSAGE_LIMIT`] messages.
//! A notification, a response and a line that is not JSON hold no request.
//!
//! [`MAX_FRAME_BYTES`]: ghostlight_relay::MAX_FRAME_BYTES

use serde_json::Value;

use super::rpc;
use crate::hold::Request;

/// The longest id, as JSON text, an answer echoes.
const ID_LIMIT: usize = 1024;

/// The longest method, as JSON text, the scanner reads.
const METHOD_LIMIT: usize = 256;

/// The longest key, as JSON text, the scanner reads: room for `"method"` written all in `\u`
/// escapes.
const KEY_LIMIT: usize = 64;

/// The most messages of a batch the scanner reads.
const MESSAGE_LIMIT: usize = 1024;

/// Which member of a message the scanner is reading the value of.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Member {
    /// The id, echoed in the answer.
    Id,
    /// The method, which picks the template.
    Method,
    /// Any other member, read past.
    #[default]
    Other,
}

/// A bounded copy of one key's or value's JSON text, without the whitespace between tokens.
#[derive(Debug, Default)]
struct Text {
    bytes: Vec<u8>,
    /// Set once the text passed its bound; it is then dropped.
    over: bool,
}

impl Text {
    /// Keep `byte`, unless that takes the text past `limit`.
    fn push(&mut self, byte: u8, limit: usize) {
        if self.over {
            return;
        }
        if self.bytes.len() == limit {
            self.over = true;
            self.bytes = Vec::new();
            return;
        }
        self.bytes.push(byte);
    }

    /// The text as one JSON value, or `None` when it passed its bound or is not JSON. Leaves the
    /// text empty.
    fn take(&mut self) -> Option<Value> {
        let over = std::mem::take(&mut self.over);
        let bytes = std::mem::take(&mut self.bytes);
        if over {
            return None;
        }
        serde_json::from_slice(&bytes).ok()
    }
}

/// Reads one skipped line, a chunk at a time, for the requests it holds.
#[derive(Debug, Default)]
pub(super) struct Scanner {
    /// How deep the scanner is in the line's value: 0 outside it.
    depth: usize,
    /// The depth of a message's members: 1 in a lone message, 2 in a batch's. 0 until the line's
    /// value opens.
    members_at: usize,
    /// Whether the scanner is inside a string.
    in_string: bool,
    /// Whether the last byte was a backslash inside a string.
    escaped: bool,
    /// Whether the scanner is inside a message it reads.
    in_message: bool,
    /// Whether the message's text being read is a key: after its `{` and after each `,`.
    in_key: bool,
    /// The member whose value is being read.
    member: Member,
    /// The key or value being read.
    text: Text,
    /// The current message's id, once read.
    id: Option<Value>,
    /// The current message's method, once read: empty when it is not a string.
    method: Option<String>,
    /// How many messages have opened.
    messages: usize,
    /// The requests read so far, in their order.
    requests: Vec<Request>,
    /// Set once nothing more is read: the line's value ended, or the line is not an object or an
    /// array.
    done: bool,
}

impl Scanner {
    /// Read the next bytes of the line.
    pub(super) fn feed(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if self.done {
                return;
            }
            self.read(byte);
        }
    }

    /// The requests the line held. A message the line never closed holds none.
    pub(super) fn finish(self) -> Vec<Request> {
        self.requests
    }

    fn read(&mut self, byte: u8) {
        if self.in_string {
            self.keep(byte);
            if self.escaped {
                self.escaped = false;
            } else if byte == b'\\' {
                self.escaped = true;
            } else if byte == b'"' {
                self.in_string = false;
            }
            return;
        }
        let members = self.in_message && self.depth == self.members_at;
        match byte {
            b' ' | b'\t' | b'\r' | b'\n' => {}
            b'{' | b'[' if self.depth == 0 => {
                self.members_at = if byte == b'{' { 1 } else { 2 };
                self.depth = 1;
                if byte == b'{' {
                    self.open_message();
                }
            }
            _ if self.depth == 0 => self.done = true,
            b'"' => {
                self.in_string = true;
                self.keep(byte);
            }
            b'{' | b'[' => {
                self.depth += 1;
                if byte == b'{' && self.depth == self.members_at {
                    self.open_message();
                } else {
                    self.keep(byte);
                }
            }
            b'}' | b']' => {
                if members {
                    self.end_member();
                    self.close_message();
                } else {
                    self.keep(byte);
                }
                self.depth -= 1;
                self.done = self.depth == 0;
            }
            b':' if members => self.end_key(),
            b',' if members => {
                self.end_member();
                self.in_key = true;
            }
            _ => self.keep(byte),
        }
    }

    /// Keep a byte of the key or of an id or method value; drop any other.
    fn keep(&mut self, byte: u8) {
        if !self.in_message {
            return;
        }
        let limit = match (self.in_key, self.member) {
            (true, _) => KEY_LIMIT,
            (false, Member::Id) => ID_LIMIT,
            (false, Member::Method) => METHOD_LIMIT,
            (false, Member::Other) => return,
        };
        self.text.push(byte, limit);
    }

    /// A message opens: read its members, unless the batch already gave its limit.
    fn open_message(&mut self) {
        self.messages += 1;
        self.in_message = self.messages <= MESSAGE_LIMIT;
        self.in_key = true;
        self.member = Member::Other;
        self.text = Text::default();
        self.id = None;
        self.method = None;
    }

    /// A key ends at its `:`; its value is read only for the id and the method.
    fn end_key(&mut self) {
        let key = self.text.take();
        self.member = match key.as_ref().and_then(Value::as_str) {
            Some(rpc::ID) => Member::Id,
            Some(rpc::METHOD) => Member::Method,
            _ => Member::Other,
        };
        self.in_key = false;
    }

    /// A member ends at its `,` or at the message's `}`. The last id or method a message gives is
    /// the one it holds, as a JSON reader keeps it.
    fn end_member(&mut self) {
        let value = self.text.take();
        if !self.in_key {
            match self.member {
                Member::Id => self.id = value,
                Member::Method => {
                    let method = value.as_ref().and_then(Value::as_str).unwrap_or_default();
                    self.method = Some(method.to_owned());
                }
                Member::Other => {}
            }
        }
        self.member = Member::Other;
    }

    /// A message closes: it holds a request when it gave both an id and a method.
    fn close_message(&mut self) {
        if let (Some(id), Some(method)) = (self.id.take(), self.method.take()) {
            self.requests.push(Request {
                key: id.to_string(),
                id,
                method,
            });
        }
        self.in_message = false;
    }
}
