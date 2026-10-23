//! Minimal Server-Sent Events reader over a reqwest byte stream.

use futures_util::StreamExt;

use crate::error::AiError;

#[derive(Debug, Clone, PartialEq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

/// Incremental parser: feed chunks, get complete events. Handles `\n` and `\r\n` line endings
/// and events split across chunks.
#[derive(Default)]
pub struct SseParser {
    buf: String,
}

impl SseParser {
    pub fn feed(&mut self, chunk: &str) -> Vec<SseEvent> {
        self.buf.push_str(&chunk.replace("\r\n", "\n"));
        let mut events = Vec::new();
        while let Some(end) = self.buf.find("\n\n") {
            let block: String = self.buf.drain(..end + 2).collect();
            if let Some(e) = parse_block(&block) {
                events.push(e);
            }
        }
        events
    }
}

fn parse_block(block: &str) -> Option<SseEvent> {
    let mut event = None;
    let mut data: Vec<&str> = Vec::new();
    for line in block.lines() {
        if let Some(v) = line.strip_prefix("event:") {
            event = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("data:") {
            data.push(v.strip_prefix(' ').unwrap_or(v));
        }
    }
    (!data.is_empty() || event.is_some()).then(|| SseEvent { event, data: data.join("\n") })
}

/// Read every event from a streaming response, calling `on_event` for each.
/// Return `Ok(false)` from the callback to stop early.
pub async fn read_events(
    resp: reqwest::Response,
    mut on_event: impl FnMut(SseEvent) -> Result<bool, AiError>,
) -> Result<(), AiError> {
    let mut parser = SseParser::default();
    let mut stream = resp.bytes_stream();
    // Multi-byte characters can straddle chunks; keep incomplete UTF-8 for the next chunk.
    let mut pending: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        pending.extend_from_slice(&chunk?);
        let valid = match std::str::from_utf8(&pending) {
            Ok(_) => pending.len(),
            Err(e) => e.valid_up_to(),
        };
        let text = String::from_utf8_lossy(&pending[..valid]).into_owned();
        pending.drain(..valid);
        for event in parser.feed(&text) {
            if !on_event(event)? {
                return Ok(());
            }
        }
    }
    for event in parser.feed("\n\n") {
        if !on_event(event)? {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_events_split_across_chunks() {
        let mut p = SseParser::default();
        assert!(p.feed("event: a\ndata: {\"x\"").is_empty());
        let events = p.feed(":1}\n\ndata: second\r\n\r\n");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event.as_deref(), Some("a"));
        assert_eq!(events[0].data, "{\"x\":1}");
        assert_eq!(events[1].event, None);
        assert_eq!(events[1].data, "second");
    }

    #[test]
    fn joins_multiline_data() {
        let mut p = SseParser::default();
        let e = p.feed("data: a\ndata: b\n\n");
        assert_eq!(e[0].data, "a\nb");
    }
}
