# Pet golden differential harness

`run.py` replays the same versioned snapshot and renderer event sequences
through the Electron compatibility contract and the Rust host contract. It
prints one JSON record per required lifecycle case with `electron`, `rust`, and
`equal` fields. The harness covers visibility, fullscreen, bounds, renderer
switching, opacity, commands, parent shutdown, and malformed snapshot repair.

Run it from the repository root:

```text
python tests/pet-golden/run.py
```

This is a deterministic bridge differential gate. Native window appearance,
WebView2/WebKitGTK composition, and real input routing are validated by the
platform E2E checks and are never inferred from this protocol-only output.
