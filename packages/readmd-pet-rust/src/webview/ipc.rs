use crate::protocol::RendererMessage;

pub fn parse_renderer_message(raw: &str) -> Option<RendererMessage> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let object = value.as_object()?;
    let kind = object.get("type")?.as_str()?.to_string();
    if kind.len() > 64 {
        return None;
    }
    let payload = object.get("payload").cloned().unwrap_or(value);
    Some(RendererMessage { kind, payload })
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
}
