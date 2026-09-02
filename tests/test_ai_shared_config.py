from src.readmd_modules import ai


def test_chat_without_explicit_provider_uses_the_saved_current_connection(monkeypatch):
    provider = {
        "id": "custom:shared",
        "name": "Shared provider",
        "base_url": "https://api.example.test/v1",
        "format": "openai",
        "mode": "chat",
        "models": ["fallback-model"],
        "credential_id": "cred:shared12345",
    }
    captured = {}
    monkeypatch.setattr(ai, "ensure_config", lambda: {
        "providers": [provider],
        "current": {"provider_id": "custom:shared", "model": "saved-model"},
    })
    monkeypatch.setattr(ai, "find_provider", lambda identifier: dict(provider) if identifier == "custom:shared" else None)
    monkeypatch.setattr(ai, "resolve_key", lambda selected: "secret" if selected.get("id") == "custom:shared" else "")
    monkeypatch.setattr(ai, "_skill_messages", lambda payload: [{"role": "user", "content": "test"}])

    def fake_chat(base_url, api_key, model, messages, temperature, stream, endpoint_mode, headers):
        captured.update(base_url=base_url, api_key=api_key, model=model, messages=messages)
        return iter(["ok"])

    monkeypatch.setattr(ai, "_chat_openai", fake_chat)

    assert list(ai.chat({"skill_id": "readmd-ask", "skill_variables": {}, "messages": [], "stream": False})) == ["ok"]
    assert captured["base_url"] == "https://api.example.test/v1"
    assert captured["api_key"] == "secret"
    assert captured["model"] == "saved-model"


def test_config_does_not_mark_missing_credential_as_ready(monkeypatch):
    provider = {
        "id": "custom:stale",
        "name": "Stale provider",
        "base_url": "https://api.example.test/v1",
        "format": "openai",
        "credential_id": "cred:stale12345",
    }
    monkeypatch.setattr(ai, "ensure_config", lambda: {
        "providers": [provider],
        "current": {"provider_id": provider["id"], "model": "model"},
    })
    monkeypatch.setattr(ai, "resolve_key", lambda _provider: "")
    monkeypatch.setattr(ai, "key_source", lambda _provider: "")
    monkeypatch.setattr(ai, "PRESETS", [])

    payload = ai.get_config()
    item = payload["custom"][0]
    assert item["has_key"] is False
    assert "credential_id" not in item
