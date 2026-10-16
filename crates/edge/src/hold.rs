//! The lines a client sent while the service was away.
//!
//! Held lines keep their order and are bounded twice: by [`HOLD_LIMIT_BYTES`] in all and by
//! [`HOLD_LIMIT`] each. A held line never reached any service, so flushing it later is its first
//! delivery, never a replay.

use std::collections::VecDeque;
use std::time::Instant;

use ghostlight_relay::{HOLD_LIMIT, HOLD_LIMIT_BYTES};
use serde_json::Value;

/// One JSON-RPC request seen in a line: its id and the method it called.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    /// The id as canonical JSON text, so `7` and `"7"` stay different requests.
    pub key: String,
    /// The id exactly as the sender wrote it, echoed in any answer.
    pub id: Value,
    /// The method the request called, which picks its failure template.
    pub method: String,
}

/// One held client line and the requests it carries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeldLine {
    /// The line, without its newline.
    pub line: String,
    /// The requests in the line. Empty for notifications, responses and unreadable lines.
    pub requests: Vec<Request>,
    /// When the line was held.
    pub held_at: Instant,
}

/// The ordered, bounded queue of held lines.
#[derive(Debug, Default)]
pub struct Hold {
    lines: VecDeque<HeldLine>,
    bytes: usize,
}

impl Hold {
    /// Hold `line` after every line already held.
    ///
    /// A line that would take the hold past [`HOLD_LIMIT_BYTES`] is not held; it comes back as
    /// the error so its requests can be answered at once.
    pub fn push(&mut self, line: HeldLine) -> Result<(), HeldLine> {
        if self.bytes + line.line.len() > HOLD_LIMIT_BYTES {
            return Err(line);
        }
        self.bytes += line.line.len();
        self.lines.push_back(line);
        Ok(())
    }

    /// Take the oldest held line, if any.
    pub fn pop(&mut self) -> Option<HeldLine> {
        let line = self.lines.pop_front()?;
        self.bytes -= line.line.len();
        Some(line)
    }

    /// Remove and return every line held for [`HOLD_LIMIT`] or longer at `now`, oldest first.
    pub fn expire(&mut self, now: Instant) -> Vec<HeldLine> {
        let mut expired = Vec::new();
        while self
            .lines
            .front()
            .is_some_and(|line| now.saturating_duration_since(line.held_at) >= HOLD_LIMIT)
        {
            expired.extend(self.pop());
        }
        expired
    }

    /// When the oldest held line expires, or `None` when nothing is held.
    #[must_use]
    pub fn next_expiry(&self) -> Option<Instant> {
        self.lines.front().map(|line| line.held_at + HOLD_LIMIT)
    }

    /// Whether any held line carries a request, which is what makes the edge start the
    /// service.
    #[must_use]
    pub fn has_request(&self) -> bool {
        self.lines.iter().any(|line| !line.requests.is_empty())
    }

    /// Remove and return every held line, oldest first.
    pub fn drain(&mut self) -> Vec<HeldLine> {
        self.bytes = 0;
        self.lines.drain(..).collect()
    }
}

#[cfg(test)]
mod tests;
