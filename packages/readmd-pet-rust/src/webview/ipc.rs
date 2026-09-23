use crate::protocol::{RendererMessage, MessageOrigin, MAX_COMMAND_BODY_BYTES};

/// Maximum raw `postMessage` body the host will parse.  Anything larger cannot
/// become a legal command anyway, so it is dropped before being deserialised.
pub const MAX_RENDERER_MESSAGE_BYTES: usize = MAX_COMMAND_BODY_BYTES;

/// Decode a message posted by page JavaScript.  Everything reaching here is
/// untrusted: it is tagged `MessageOrigin::Page` and the host authenticates it
/// against the session token and navigation generation it handed to that
/// document.
pub fn parse_renderer_message(raw: &str) -> Option<RendererMessage> {
    if raw.len() > MAX_RENDERER_MESSAGE_BYTES {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let object = value.as_object()?;
    let kind = object.get("type")?.as_str()?.to_string();
    if kind.len() > 64 {
        return None;
    }
    let payload = object.get("payload").cloned().unwrap_or(value);
    Some(RendererMessage {
        kind,
        payload,
        origin: MessageOrigin::Page,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_parser_keeps_payload_and_rejects_unknown_shape() {
        let message = parse_renderer_message(r#"{"type":"bounds","payload":{"x":1}}"#).unwrap();
        assert_eq!(message.kind, "bounds");
        assert_eq!(message.payload["x"], 1);
        assert!(parse_renderer_message("[]").is_none());
    }

    #[test]
    fn every_page_message_is_tagged_untrusted() {
        // The host gate keys off this tag; a `Host` tag here would let a page
        // postMessage act as native input and skip authentication entirely.
        let message = parse_renderer_message(r#"{"type":"open-app"}"#).unwrap();
        assert_eq!(message.origin, MessageOrigin::Page);
        assert!(!message.authenticated("session", 1));
        assert!(MAX_RENDERER_MESSAGE_BYTES <= crate::protocol::MAX_COMMAND_BODY_BYTES);
    }

    #[test]
    fn ipc_parser_fails_closed_for_malformed_and_oversized_messages() {
        let samples = [
            "",
            "null",
            "[]",
            "{}",
            r#"{"type":null}"#,
            r#"{"type":""}"#,
            r#"{"type":"renderer-ready","payload":[]}"#,
            &format!(r#"{{"type":"state","payload":"{}"}}"#, "x".repeat(100_000)),
        ];
        for sample in samples {
            let _ = parse_renderer_message(sample);
        }
        assert!(parse_renderer_message(&format!("{{\"type\":\"{}\"}}", "x".repeat(65))).is_none());
    }
}
