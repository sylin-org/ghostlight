//! Tests of how held lines are kept, refused and expired.

use std::time::Duration;

use serde_json::json;

use super::*;

fn line(text: &str, held_at: Instant, requests: Vec<Request>) -> HeldLine {
    HeldLine {
        line: text.to_owned(),
        requests,
        held_at,
    }
}

fn request(id: u32) -> Request {
    Request {
        key: id.to_string(),
        id: json!(id),
        method: "tools/call".to_owned(),
    }
}

#[test]
fn lines_come_back_in_the_order_they_were_held() {
    let now = Instant::now();
    let mut hold = Hold::default();
    hold.push(line("a", now, vec![])).unwrap();
    hold.push(line("b", now, vec![])).unwrap();
    hold.push(line("c", now, vec![])).unwrap();
    assert_eq!(hold.pop().unwrap().line, "a");
    let order: Vec<_> = hold.drain().into_iter().map(|held| held.line).collect();
    assert_eq!(order, ["b", "c"]);
    assert!(hold.pop().is_none());
}

#[test]
fn a_line_past_the_byte_limit_is_refused() {
    let now = Instant::now();
    let mut hold = Hold::default();
    hold.push(line(&"x".repeat(HOLD_LIMIT_BYTES - 1), now, vec![]))
        .unwrap();
    let refused = hold.push(line("yz", now, vec![request(1)])).unwrap_err();
    assert_eq!(refused.requests, [request(1)]);
    hold.push(line("y", now, vec![])).unwrap();
    hold.drain();
    hold.push(line("yz", now, vec![])).unwrap();
}

#[test]
fn lines_expire_at_the_time_limit_oldest_first() {
    let start = Instant::now();
    let mut hold = Hold::default();
    hold.push(line("a", start, vec![])).unwrap();
    hold.push(line("b", start + Duration::from_secs(10), vec![]))
        .unwrap();
    assert_eq!(hold.next_expiry(), Some(start + HOLD_LIMIT));
    assert!(hold
        .expire(start + HOLD_LIMIT - Duration::from_millis(1))
        .is_empty());
    let expired = hold.expire(start + HOLD_LIMIT);
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].line, "a");
    assert_eq!(
        hold.next_expiry(),
        Some(start + Duration::from_secs(10) + HOLD_LIMIT)
    );
}

#[test]
fn only_a_held_request_counts_as_a_request() {
    let now = Instant::now();
    let mut hold = Hold::default();
    hold.push(line("notification", now, vec![])).unwrap();
    assert!(!hold.has_request());
    hold.push(line("request", now, vec![request(1)])).unwrap();
    assert!(hold.has_request());
}
