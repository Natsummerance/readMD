// -*- coding: utf-8 -*-
//! Native-window capability probe for a future renderer adapter.
//!
//! Authority: `src/readmd_modules/pet/window_adapter.py` (all 9 defs), plus its
//! only caller `src/readmd_modules/pet/probe.py:11-20`.
//!
//! | Python callable | Lines | Rust item |
//! |---|---|---|
//! | `NativePetProbe.probe_capabilities` | 22-49 | [`probe_capabilities`] |
//! | `NativePetProbe.create_probe_window` | 51-68 | [`create_probe_window`] |
//! | `NativePetProbe.probe_html` | 70-85 | [`probe_html`] |
//! | `PetProbeDragBridge.__init__` | 97-99 | [`PetProbeDragBridge::new`] |
//! | `PetProbeDragBridge.bind` | 101-102 | [`PetProbeDragBridge::bind`] |
//! | `PetProbeDragBridge.begin_drag` | 104-108 | [`PetProbeDragBridge::begin_drag`] |
//! | `PetProbeDragBridge.move_drag` | 110-117 | [`PetProbeDragBridge::move_drag`] |
//! | `PetProbeDragBridge.end_drag` | 119-121 | [`PetProbeDragBridge::end_drag`] |
//! | `PetProbeDragBridge._require_window` | 123-126 | [`PetProbeDragBridge::require_window`] |
//! | `_report` | 129-139 | [`report`] |
//!
//! # How `inspect.signature` becomes data
//!
//! `probe_capabilities` introspects a *Python* callable, which a native binary
//! cannot do.  Everything the introspection can observe is flattened into
//! [`WebviewModule`]: the two `callable(getattr(...))` facts, whether
//! `create_window` takes `**kwargs`, and its parameter names.
//! [`WebviewModule::pywebview()] holds the table measured from the vendored
//! pywebview (`webview.create_window` has 34 named parameters, no `**kwargs`),
//! so the gate is evaluated against real data instead of a guess.  A
//! `signature_error` flag stands in for the `TypeError`/`ValueError` branch at
//! `window_adapter.py:37-38`.
//!
//! # Window creation
//!
//! `create_probe_window` hands its keyword arguments to `pywebview`, which a
//! native kernel must not link.  The port keeps the whole observable contract —
//! the `native_window` gate, the exact keyword set, the HTML, the bridge
//! binding and the `_pet_probe_bridge` attribute — and represents the toolkit
//! call by constructing a [`ProbeWindowSpec`] over a [`PetProbeWindow`]
//! implementation.  Opening a real top-level window additionally needs an event
//! loop the probe does not own, so that last step stays a seam; the arithmetic
//! the probe exists to verify (`PetProbeDragBridge`) is fully implemented.

use serde_json::Value;

use crate::pet_launcher::{py_float_value, py_truthy, OrdValue};

/// `window_adapter.py:48` / `:39` / `:30` — the codes the report carries.
pub const CODE_WINDOWS_OVERLAY_REQUIRED: &str = "windows_overlay_adapter_required";
pub const CODE_MANUAL_VERIFICATION_REQUIRED: &str = "manual-verification-required";
pub const CODE_UNAVAILABLE: &str = "unavailable";
pub const CODE_UNKNOWN: &str = "unknown";

/// `raise RuntimeError("native_pet_window_capability_unavailable")`
/// (`window_adapter.py:55`).
pub const ERROR_CAPABILITY_UNAVAILABLE: &str = "native_pet_window_capability_unavailable";
/// `raise RuntimeError("pet_probe_window_unavailable")` (`window_adapter.py:125`).
pub const ERROR_WINDOW_UNAVAILABLE: &str = "pet_probe_window_unavailable";
/// `{"ok": False, "code": "drag_not_started"}` (`window_adapter.py:112`).
pub const CODE_DRAG_NOT_STARTED: &str = "drag_not_started";

/// `sys.platform` (`window_adapter.py:24`), the default when no platform name
/// is supplied.
pub fn sys_platform() -> &'static str {
    if cfg!(windows) {
        "win32"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    }
}

/// The observable facts `probe_capabilities` reads off a webview module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebviewModule {
    /// `callable(getattr(webview_module, "create_window", None))` (`:25-27`).
    pub has_create_window: bool,
    /// `callable(getattr(webview_module, "start", None))` (`:26-27`).
    pub has_start: bool,
    /// `any(param.kind == inspect.Parameter.VAR_KEYWORD ...)` (`:32`).
    pub accepts_var_keyword: bool,
    /// `signature.parameters` (`:31`), in declaration order.
    pub parameters: Vec<String>,
    /// `inspect.signature(create_window)` raised `TypeError`/`ValueError`
    /// (`:37-38`).
    pub signature_error: bool,
}

impl WebviewModule {
    /// The vendored pywebview module, measured with
    /// `python -c "import inspect, webview; print(inspect.signature(webview.create_window))"`.
    /// Every one of the three probed names (`transparent`, `on_top`,
    /// `frameless`) is present, which is exactly why the Windows transparency
    /// comment at `window_adapter.py:39-44` exists: the signature says yes, the
    /// backend contract says no.
    pub fn pywebview() -> WebviewModule {
        const PARAMETERS: [&str; 32] = [
            "title",
            "url",
            "html",
            "js_api",
            "width",
            "height",
            "x",
            "y",
            "screen",
            "resizable",
            "fullscreen",
            "min_size",
            "hidden",
            "frameless",
            "easy_drag",
            "shadow",
            "focus",
            "minimized",
            "maximized",
            "on_top",
            "confirm_close",
            "background_color",
            "transparent",
            "text_select",
            "zoomable",
            "draggable",
            "vibrancy",
            "menu",
            "localization",
            "server",
            "http_port",
            "server_args",
        ];
        WebviewModule {
            has_create_window: true,
            has_start: true,
            accepts_var_keyword: false,
            parameters: PARAMETERS.iter().map(|name| name.to_string()).collect(),
            signature_error: false,
        }
    }

    /// `lambda name: accepts_keywords or name in parameters`
    /// (`window_adapter.py:33`).
    pub fn supports(&self, name: &str) -> bool {
        self.accepts_var_keyword || self.parameters.iter().any(|parameter| parameter == name)
    }
}

/// `NativePetProbe.probe_capabilities(webview_module, platform_name=None)`
/// (`window_adapter.py:22-49`).
pub fn probe_capabilities(module: &WebviewModule, platform_name: Option<&str>) -> OrdValue {
    // `platform_name = platform_name or sys.platform` (`:24`) — Python's
    // `or`, so an empty string also falls back.
    let platform_name = match platform_name {
        Some(text) if !text.is_empty() => text.to_string(),
        _ => sys_platform().to_string(),
    };
    if !module.has_create_window || !module.has_start {
        return report(&platform_name, false, false, false, CODE_UNAVAILABLE);
    }
    if module.signature_error {
        return report(&platform_name, false, false, false, CODE_UNKNOWN);
    }
    let transparent = module.supports("transparent");
    let on_top = module.supports("on_top");
    let frameless = module.supports("frameless");
    // pywebview exposes `transparent` in its Python signature on Windows, but
    // its own backend contract says transparent windows are not supported
    // there, so a signature check alone is a false positive
    // (`window_adapter.py:39-44`).
    let windows_transparency = platform_name.starts_with("win");
    let transparent_window = transparent && !windows_transparency;
    let native_window = transparent_window && on_top && frameless;
    let code =
        if windows_transparency { CODE_WINDOWS_OVERLAY_REQUIRED } else { CODE_MANUAL_VERIFICATION_REQUIRED };
    report(&platform_name, native_window, transparent_window, on_top, code)
}

/// `_report(platform_name, native_window, transparent, on_top, click_through)`
/// (`window_adapter.py:129-139`).  The fifth parameter is still named
/// `click_through` while every caller passes a capability *code* — ported
/// verbatim, including the key name.
pub fn report(
    platform_name: &str,
    native_window: bool,
    transparent: bool,
    on_top: bool,
    click_through: &str,
) -> OrdValue {
    OrdValue::object(vec![
        ("platform", OrdValue::text(platform_name)),
        ("native_window", OrdValue::boolean(native_window)),
        ("transparent_window", OrdValue::boolean(transparent)),
        ("always_on_top", OrdValue::boolean(on_top)),
        ("click_through", OrdValue::text(click_through)),
        ("drag_drop", OrdValue::text(CODE_MANUAL_VERIFICATION_REQUIRED)),
        ("multi_monitor", OrdValue::text(CODE_MANUAL_VERIFICATION_REQUIRED)),
        ("release_ready", OrdValue::boolean(false)),
    ])
}

/// The window the drag bridge moves — pywebview's `Window.x`, `.y` and
/// `.move(x, y)` (`window_adapter.py:106-116`).
pub trait PetProbeWindow {
    fn x(&self) -> i64;
    fn y(&self) -> i64;
    fn move_to(&mut self, x: i64, y: i64);
}

/// The manual-diagnostics stand-in: records what the bridge asked the window to
/// do, which is precisely what the probe is there to make visible.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ManualProbeWindow {
    pub x: i64,
    pub y: i64,
    pub moves: Vec<(i64, i64)>,
}

impl PetProbeWindow for ManualProbeWindow {
    fn x(&self) -> i64 {
        self.x
    }
    fn y(&self) -> i64 {
        self.y
    }
    fn move_to(&mut self, x: i64, y: i64) {
        self.x = x;
        self.y = y;
        self.moves.push((x, y));
    }
}

/// `raise RuntimeError(...)` for the two places `window_adapter.py` raises.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeError(pub &'static str);

impl std::fmt::Display for ProbeError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "{}", self.0)
    }
}

impl std::error::Error for ProbeError {}

/// `class PetProbeDragBridge` (`window_adapter.py:88`).
pub struct PetProbeDragBridge {
    window: Option<Box<dyn PetProbeWindow>>,
    offset: Option<(i64, i64)>,
}

impl Default for PetProbeDragBridge {
    fn default() -> Self {
        PetProbeDragBridge::new(None)
    }
}

impl PetProbeDragBridge {
    /// `PetProbeDragBridge(window=None)` (`window_adapter.py:97-99`).
    pub fn new(window: Option<Box<dyn PetProbeWindow>>) -> PetProbeDragBridge {
        PetProbeDragBridge { window, offset: None }
    }

    /// `bind(window)` (`window_adapter.py:101-102`).
    pub fn bind(&mut self, window: Box<dyn PetProbeWindow>) {
        self.window = Some(window);
    }

    pub fn is_bound(&self) -> bool {
        self.window.is_some()
    }

    pub fn offset(&self) -> Option<(i64, i64)> {
        self.offset
    }

    /// `begin_drag(screen_x, screen_y)` (`window_adapter.py:104-108`).
    ///
    /// The arguments are whatever JS sent (`e.screenX`, `e.screenY`), so they
    /// go through `float()` first: `int(round(float(screen_x))) -
    /// int(window.x)`.
    pub fn begin_drag(&mut self, screen_x: &Value, screen_y: &Value) -> Result<OrdValue, ProbeError> {
        let window = self.require_window()?;
        let origin_x = rounded_screen(screen_x)?;
        let origin_y = rounded_screen(screen_y)?;
        self.offset = Some((origin_x - window.x(), origin_y - window.y()));
        Ok(OrdValue::object(vec![("ok", OrdValue::boolean(true))]))
    }

    /// `move_drag(screen_x, screen_y)` (`window_adapter.py:110-117`).
    pub fn move_drag(&mut self, screen_x: &Value, screen_y: &Value) -> Result<OrdValue, ProbeError> {
        let (offset_x, offset_y) = match self.offset {
            None => {
                return Ok(OrdValue::object(vec![
                    ("ok", OrdValue::boolean(false)),
                    ("code", OrdValue::text(CODE_DRAG_NOT_STARTED)),
                ]))
            }
            Some(offset) => offset,
        };
        let window = self.require_window()?;
        let x = rounded_screen(screen_x)? - offset_x;
        let y = rounded_screen(screen_y)? - offset_y;
        window.move_to(x, y);
        Ok(OrdValue::object(vec![
            ("ok", OrdValue::boolean(true)),
            ("x", OrdValue::int(x)),
            ("y", OrdValue::int(y)),
        ]))
    }

    /// `end_drag(*_unused)` (`window_adapter.py:119-121`) — never touches the
    /// window, so an unbound bridge still returns `{"ok": True}`.
    pub fn end_drag(&mut self) -> OrdValue {
        self.offset = None;
        OrdValue::object(vec![("ok", OrdValue::boolean(true))])
    }

    /// `_require_window()` (`window_adapter.py:123-126`).
    pub fn require_window(&mut self) -> Result<&mut Box<dyn PetProbeWindow>, ProbeError> {
        if self.window.is_none() {
            return Err(ProbeError(ERROR_WINDOW_UNAVAILABLE));
        }
        Ok(self.window.as_mut().expect("window presence checked above"))
    }
}

/// The window is behind a `dyn` object, so `Debug` cannot be derived.
impl std::fmt::Debug for PetProbeDragBridge {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("PetProbeDragBridge")
            .field("bound", &self.window.is_some())
            .field("offset", &self.offset)
            .finish()
    }
}

/// `int(round(float(value)))` (`window_adapter.py:106-107`, `:114-115`).
/// `round()` without a second argument is banker's rounding; a non-coercible or
/// infinite value leaves Python raising, which this reports as an error.
fn rounded_screen(value: &Value) -> Result<i64, ProbeError> {
    match py_float_value(value) {
        None => Err(ProbeError("float() failed")),
        Some(number) => {
            if !number.is_finite() {
                return Err(ProbeError("int(round(infinite)) overflow"));
            }
            Ok(crate::pdf_editor::py_round_int(number))
        }
    }
}

/// The keyword arguments `create_probe_window` hands to `pywebview`
/// (`window_adapter.py:57-63`), kept as data because a native kernel does not
/// link a third-party windowing binding.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeWindowSpec {
    pub title: String,
    pub html: String,
    pub width: i64,
    pub height: i64,
    pub min_size: (i64, i64),
    pub frameless: bool,
    pub on_top: bool,
    pub transparent: bool,
    pub text_select: bool,
    pub zoomable: bool,
    pub draggable: bool,
    pub shadow: bool,
    pub background_color: String,
}

/// A probe window plus the bridge bound to it; `setattr(window,
/// "_pet_probe_bridge", bridge)` (`window_adapter.py:67`).
pub struct ProbeWindow {
    pub spec: ProbeWindowSpec,
    pub bridge: PetProbeDragBridge,
}

impl std::fmt::Debug for ProbeWindow {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("ProbeWindow")
            .field("spec", &self.spec)
            .field("bridge", &self.bridge)
            .finish()
    }
}

/// `NativePetProbe.create_probe_window(webview_module, platform_name=None)`
/// (`window_adapter.py:51-68`).
pub fn create_probe_window(
    module: &WebviewModule,
    platform_name: Option<&str>,
) -> Result<ProbeWindow, ProbeError> {
    let report = probe_capabilities(module, platform_name);
    let native_window = report.is_true("native_window");
    if !native_window {
        return Err(ProbeError(ERROR_CAPABILITY_UNAVAILABLE));
    }
    let mut bridge = PetProbeDragBridge::new(None);
    bridge.bind(Box::new(ManualProbeWindow::default()));
    Ok(ProbeWindow {
        spec: ProbeWindowSpec {
            title: "ReadMD".to_string(),
            html: probe_html().to_string(),
            width: 220,
            height: 240,
            min_size: (160, 160),
            frameless: true,
            on_top: true,
            transparent: true,
            text_select: false,
            zoomable: false,
            draggable: false,
            shadow: false,
            background_color: "#000000".to_string(),
        },
        bridge,
    })
}

/// `NativePetProbe.probe_html()` (`window_adapter.py:70-85`) — byte for byte.
/// This is not a character or a replacement for a Live2D model; it only makes
/// transparency, positioning and drag handling visible to a tester.
pub fn probe_html() -> &'static str {
    r##"<!doctype html><meta charset="utf-8"><style>
html,body{margin:0;width:100%;height:100%;background:transparent;overflow:hidden}
#probe{width:100%;height:100%;border-radius:50%;background:radial-gradient(circle at 35% 28%,#c8d8ff,#5c84f7 58%,#263764);box-sizing:border-box;border:1px solid rgba(255,255,255,.76);box-shadow:0 8px 26px rgba(0,0,0,.35);touch-action:none;user-select:none;cursor:grab}
#probe.dragging{cursor:grabbing}
</style><div id="probe" aria-label="ReadMD native pet capability probe"></div><script>
(()=>{const p=document.getElementById('probe');let active=false;const api=()=>window.pywebview&&window.pywebview.api;
const call=(name,e)=>{const bridge=api();if(bridge&&bridge[name]){Promise.resolve(bridge[name](e.screenX,e.screenY)).catch(()=>{});}};
p.addEventListener('pointerdown',e=>{active=true;p.classList.add('dragging');p.setPointerCapture?.(e.pointerId);call('begin_drag',e);});
p.addEventListener('pointermove',e=>{if(active)call('move_drag',e);});
const stop=e=>{if(!active)return;active=false;p.classList.remove('dragging');call('end_drag',e);p.releasePointerCapture?.(e.pointerId);};
p.addEventListener('pointerup',stop);p.addEventListener('pointercancel',stop);})();
</script>"##
}

/// `probe.py:16` — `if not report["native_window"]: return 2`.
pub fn probe_exit_code(report: &OrdValue) -> i32 {
    let native_window = match report.get("native_window") {
        Some(value) => py_truthy(&value.to_json()),
        None => false,
    };
    if native_window {
        0
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::sha256_hex;
    use serde_json::json;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    /// The `probe_capabilities` report rendered the way `_report` builds it.
    fn rendered(module: &WebviewModule, platform: Option<&str>) -> String {
        probe_capabilities(module, platform).dump()
    }

    /// A module whose `create_window` accepts exactly `parameters`.
    fn module_with(parameters: &[&str]) -> WebviewModule {
        WebviewModule {
            has_create_window: true,
            has_start: true,
            accepts_var_keyword: false,
            parameters: parameters.iter().map(|name| name.to_string()).collect(),
            signature_error: false,
        }
    }

    fn full_module() -> WebviewModule {
        module_with(&["transparent", "on_top", "frameless"])
    }

    /// The eight keys `_report` writes, in order, with the caller-dependent ones
    /// substituted.  Used to pin the byte layout of every capability report.
    fn expected(platform: &str, native: &str, transparent: &str, on_top: &str, code: &str) -> String {
        format!(
            "{{\"platform\":\"{platform}\",\"native_window\":{native},\"transparent_window\":{transparent},\
             \"always_on_top\":{on_top},\"click_through\":\"{code}\",\"drag_drop\":\"manual-verification-required\",\
             \"multi_monitor\":\"manual-verification-required\",\"release_ready\":false}}"
        )
    }

    /// Records every move the bridge asks for, through the `Rc` so the test can
    /// read it back after the window has been moved into the bridge.
    #[derive(Clone, Default)]
    struct SpyWindow(Rc<Spy>);

    #[derive(Default)]
    struct Spy {
        x: Cell<i64>,
        y: Cell<i64>,
        moves: RefCell<Vec<(i64, i64)>>,
    }

    impl PetProbeWindow for SpyWindow {
        fn x(&self) -> i64 {
            self.0.x.get()
        }
        fn y(&self) -> i64 {
            self.0.y.get()
        }
        fn move_to(&mut self, x: i64, y: i64) {
            self.0.x.set(x);
            self.0.y.set(y);
            self.0.moves.borrow_mut().push((x, y));
        }
    }

    impl SpyWindow {
        fn at(x: i64, y: i64) -> (SpyWindow, Rc<Spy>) {
            let spy = Rc::new(Spy {
                x: Cell::new(x),
                y: Cell::new(y),
                moves: RefCell::new(Vec::new()),
            });
            (SpyWindow(Rc::clone(&spy)), spy)
        }
    }

    // ------------------------------------------------------ probe_capabilities

    #[test]
    fn missing_create_window_is_unavailable() {
        let module = WebviewModule {
            has_create_window: false,
            has_start: true,
            accepts_var_keyword: false,
            parameters: vec![],
            signature_error: false,
        };
        assert_eq!(
            rendered(&module, Some("win32")),
            expected("win32", "false", "false", "false", "unavailable")
        );
    }

    #[test]
    fn missing_start_is_unavailable_even_with_a_full_signature() {
        let mut module = full_module();
        module.has_start = false;
        assert_eq!(
            rendered(&module, Some("darwin")),
            expected("darwin", "false", "false", "false", "unavailable")
        );
    }

    #[test]
    fn non_callable_attributes_are_unavailable_not_unknown() {
        // `callable(getattr(webview_module, "create_window", None))` is False for
        // a plain string attribute, so the signature is never inspected.
        let module = WebviewModule {
            has_create_window: false,
            has_start: false,
            accepts_var_keyword: true,
            parameters: vec![],
            signature_error: true,
        };
        assert_eq!(
            rendered(&module, Some("linux")),
            expected("linux", "false", "false", "false", "unavailable")
        );
    }

    #[test]
    fn signature_probe_failure_is_unknown() {
        let mut module = full_module();
        module.signature_error = true;
        assert_eq!(
            rendered(&module, Some("win32")),
            expected("win32", "false", "false", "false", "unknown")
        );
    }

    #[test]
    fn windows_signature_is_a_transparency_false_positive() {
        // window_adapter.py:39-44: the signature says `transparent`, the backend
        // contract says no, so `transparent_window` and `native_window` are both
        // False while `always_on_top` keeps the raw signature answer.
        assert_eq!(
            rendered(&full_module(), Some("win32")),
            expected(
                "win32",
                "false",
                "false",
                "true",
                "windows_overlay_adapter_required"
            )
        );
    }

    #[test]
    fn any_win_prefixed_platform_takes_the_overlay_branch() {
        for platform in ["win32", "win", "windows", "win-x64"] {
            let report = probe_capabilities(&full_module(), Some(platform));
            assert_eq!(
                report.get("click_through"),
                Some(&OrdValue::text(CODE_WINDOWS_OVERLAY_REQUIRED)),
                "{platform}"
            );
            assert_eq!(report.get("native_window"), Some(&OrdValue::boolean(false)));
        }
    }

    #[test]
    fn platform_prefix_test_is_case_sensitive() {
        // `"Win32".startswith("win")` is False in Python.
        assert_eq!(
            rendered(&full_module(), Some("Win32")),
            expected("Win32", "true", "true", "true", "manual-verification-required")
        );
    }

    #[test]
    fn darwin_with_all_three_keywords_is_native() {
        assert_eq!(
            rendered(&full_module(), Some("darwin")),
            expected("darwin", "true", "true", "true", "manual-verification-required")
        );
    }

    #[test]
    fn each_missing_keyword_clears_native_window_but_keeps_the_code() {
        for missing in ["transparent", "on_top", "frameless"] {
            let parameters: Vec<&str> = ["transparent", "on_top", "frameless"]
                .into_iter()
                .filter(|name| *name != missing)
                .collect();
            let report = probe_capabilities(&module_with(&parameters), Some("darwin"));
            assert_eq!(report.get("native_window"), Some(&OrdValue::boolean(false)), "{missing}");
            let transparent_expected = if missing == "transparent" { "false" } else { "true" };
            assert_eq!(
                report.dump(),
                expected(
                    "darwin",
                    "false",
                    transparent_expected,
                    if missing == "on_top" { "false" } else { "true" },
                    "manual-verification-required"
                ),
                "{missing}"
            );
        }
    }

    #[test]
    fn var_keyword_modules_support_every_name() {
        let module = WebviewModule {
            has_create_window: true,
            has_start: true,
            accepts_var_keyword: true,
            parameters: vec![],
            signature_error: false,
        };
        assert!(module.supports("transparent"));
        assert!(module.supports("anything_at_all"));
        assert_eq!(
            rendered(&module, Some("linux")),
            expected("linux", "true", "true", "true", "manual-verification-required")
        );
    }

    #[test]
    fn supports_is_a_plain_membership_test() {
        let module = module_with(&["transparent", "on_top", "frameless", "easy_drag"]);
        assert!(module.supports("easy_drag"));
        assert!(!module.supports("shadow"));
        assert!(!module.supports("Transparent"));
        assert!(!module.supports(""));
    }

    #[test]
    fn empty_platform_name_falls_back_like_python_or() {
        let report = probe_capabilities(&full_module(), Some(""));
        assert_eq!(
            report.get("platform"),
            Some(&OrdValue::text(sys_platform())),
            "`platform_name or sys.platform`"
        );
    }

    #[test]
    fn absent_platform_name_uses_sys_platform() {
        let report = probe_capabilities(&full_module(), None);
        let platform = match report.get("platform") {
            Some(OrdValue::Json(Value::String(text))) => text.clone(),
            other => panic!("platform is {other:?}"),
        };
        assert_eq!(platform, sys_platform());
        assert_eq!(
            report.get("click_through"),
            Some(&OrdValue::text(if platform.starts_with("win") {
                CODE_WINDOWS_OVERLAY_REQUIRED
            } else {
                CODE_MANUAL_VERIFICATION_REQUIRED
            }))
        );
    }

    #[test]
    fn sys_platform_is_the_python_name_for_this_target() {
        if cfg!(windows) {
            assert_eq!(sys_platform(), "win32");
        } else if cfg!(target_os = "macos") {
            assert_eq!(sys_platform(), "darwin");
        } else {
            assert_eq!(sys_platform(), "linux");
        }
    }

    // ------------------------------------------------------------------ report

    #[test]
    fn report_has_the_eight_python_keys_in_order() {
        let report = report("linux", true, true, false, "manual-verification-required");
        assert_eq!(
            report.dump(),
            "{\"platform\":\"linux\",\"native_window\":true,\"transparent_window\":true,\"always_on_top\":false,\
             \"click_through\":\"manual-verification-required\",\"drag_drop\":\"manual-verification-required\",\
             \"multi_monitor\":\"manual-verification-required\",\"release_ready\":false}"
        );
    }

    #[test]
    fn report_never_admits_release_readiness() {
        for platform in ["win32", "darwin", "linux"] {
            let report = probe_capabilities(&full_module(), Some(platform));
            assert_eq!(report.get("release_ready"), Some(&OrdValue::boolean(false)));
            assert_eq!(
                report.get("drag_drop"),
                Some(&OrdValue::text(CODE_MANUAL_VERIFICATION_REQUIRED))
            );
            assert_eq!(
                report.get("multi_monitor"),
                Some(&OrdValue::text(CODE_MANUAL_VERIFICATION_REQUIRED))
            );
        }
    }

    #[test]
    fn vendored_pywebview_signature_is_the_measured_table() {
        let module = WebviewModule::pywebview();
        assert!(module.has_create_window && module.has_start);
        assert!(!module.accepts_var_keyword);
        assert!(!module.signature_error);
        assert_eq!(module.parameters.len(), 32);
        assert_eq!(module.parameters[0], "title");
        assert_eq!(module.parameters[module.parameters.len() - 1], "server_args");
        for name in ["transparent", "on_top", "frameless", "min_size", "background_color"] {
            assert!(module.supports(name), "{name}");
        }
        // pywebview has no `click_through` keyword, which is why the report's
        // fifth field carries a code rather than a capability.
        assert!(!module.supports("click_through"));
    }

    // ----------------------------------------------------- PetProbeDragBridge

    #[test]
    fn begin_drag_needs_a_window() {
        let mut bridge = PetProbeDragBridge::new(None);
        assert!(!bridge.is_bound());
        assert_eq!(
            bridge.begin_drag(&json!(10), &json!(20)),
            Err(ProbeError(ERROR_WINDOW_UNAVAILABLE))
        );
    }

    #[test]
    fn move_drag_checks_the_offset_before_the_window() {
        // `if self._offset is None` runs before `_require_window()`, so an
        // unbound bridge answers `drag_not_started` instead of raising.
        let mut bridge = PetProbeDragBridge::new(None);
        assert_eq!(
            bridge.move_drag(&json!(10), &json!(20)).expect("dict").dump(),
            "{\"ok\":false,\"code\":\"drag_not_started\"}"
        );
    }

    #[test]
    fn drag_offsets_are_relative_to_the_screen() {
        let (window, spy) = SpyWindow::at(100, 50);
        let mut bridge = PetProbeDragBridge::new(Some(Box::new(window)));
        assert_eq!(bridge.begin_drag(&json!(150), &json!(80)).expect("ok").dump(), "{\"ok\":true}");
        assert_eq!(bridge.offset(), Some((50, 30)));
        assert_eq!(
            bridge.move_drag(&json!(200), &json!(100)).expect("moved").dump(),
            "{\"ok\":true,\"x\":150,\"y\":70}"
        );
        assert_eq!(spy.moves.borrow().as_slice(), &[(150, 70)]);
        assert_eq!((spy.x.get(), spy.y.get()), (150, 70));
        // The window is not moved again by a second read of the same offset.
        assert_eq!(
            bridge.move_drag(&json!(120), &json!(60)).expect("moved").dump(),
            "{\"ok\":true,\"x\":70,\"y\":30}"
        );
        assert_eq!(spy.moves.borrow().as_slice(), &[(150, 70), (70, 30)]);
    }

    #[test]
    fn drag_origin_rounds_half_to_even() {
        for (screen, want_offset) in [
            (150.5, 50i64),  // round(150.5) == 150
            (151.5, 152 - 100), // round(151.5) == 152
            (150.6, 51),     // round(150.6) == 151
            (149.4, 49),     // round(149.4) == 149
            (-149.5, -250),  // round(-149.5) == -150, window at 100
        ] {
            let (window, _spy) = SpyWindow::at(100, 50);
            let mut bridge = PetProbeDragBridge::new(Some(Box::new(window)));
            bridge.begin_drag(&json!(screen), &json!(50)).expect("ok");
            assert_eq!(bridge.offset().expect("offset").0, want_offset, "{screen}");
        }
    }

    #[test]
    fn drag_accepts_the_strings_js_may_send() {
        let (window, spy) = SpyWindow::at(0, 0);
        let mut bridge = PetProbeDragBridge::new(Some(Box::new(window)));
        bridge.begin_drag(&json!("150"), &json!(" 80 ")).expect("ok");
        assert_eq!(bridge.offset(), Some((150, 80)));
        assert_eq!(
            bridge.move_drag(&json!("\u{a0}200"), &json!("1\u{665}0")).expect("moved").dump(),
            "{\"ok\":true,\"x\":50,\"y\":70}"
        );
        assert_eq!(spy.moves.borrow().as_slice(), &[(50, 70)]);
        // Booleans are numbers to `float()`.
        let (window, _spy) = SpyWindow::at(10, 10);
        let mut bridge = PetProbeDragBridge::new(Some(Box::new(window)));
        bridge.begin_drag(&json!(true), &json!(false)).expect("ok");
        assert_eq!(bridge.offset(), Some((-9, -10)));
    }

    #[test]
    fn drag_reports_values_that_float_cannot_parse() {
        for bad in [json!(""), json!("abc"), json!(null), json!([]), json!({}), json!("1.2.3")] {
            let (window, _spy) = SpyWindow::at(0, 0);
            let mut bridge = PetProbeDragBridge::new(Some(Box::new(window)));
            assert_eq!(bridge.begin_drag(&bad, &json!(0)), Err(ProbeError("float() failed")), "{bad}");
        }
        for infinite in [json!("inf"), json!("NaN"), json!("1e999")] {
            let (window, _spy) = SpyWindow::at(0, 0);
            let mut bridge = PetProbeDragBridge::new(Some(Box::new(window)));
            // Python raises here too (`ValueError` for `inf`,
            // `OverflowError` for `round(inf)`); both are errors, not a move.
            assert_eq!(
                bridge.begin_drag(&infinite, &json!(0)),
                Err(ProbeError("int(round(infinite)) overflow")),
                "{infinite}"
            );
        }
    }

    #[test]
    fn end_drag_clears_the_offset() {
        let (window, spy) = SpyWindow::at(10, 10);
        let mut bridge = PetProbeDragBridge::new(Some(Box::new(window)));
        bridge.begin_drag(&json!(20), &json!(20)).expect("ok");
        assert_eq!(bridge.end_drag().dump(), "{\"ok\":true}");
        assert_eq!(bridge.offset(), None);
        assert_eq!(
            bridge.move_drag(&json!(30), &json!(30)).expect("dict").dump(),
            "{\"ok\":false,\"code\":\"drag_not_started\"}"
        );
        assert_eq!(spy.moves.borrow().len(), 0, "end_drag never moves the window");
    }

    #[test]
    fn end_drag_works_without_a_window() {
        let mut bridge = PetProbeDragBridge::new(None);
        assert_eq!(bridge.end_drag().dump(), "{\"ok\":true}");
    }

    #[test]
    fn bind_replaces_the_window_the_bridge_moves() {
        let (first, first_spy) = SpyWindow::at(1000, 1000);
        let (second, second_spy) = SpyWindow::at(5, 5);
        let mut bridge = PetProbeDragBridge::default();
        bridge.bind(Box::new(first));
        bridge.begin_drag(&json!(1005), &json!(1005)).expect("ok");
        assert_eq!(bridge.offset(), Some((5, 5)));
        bridge.bind(Box::new(second));
        assert_eq!(
            bridge.move_drag(&json!(9), &json!(9)).expect("moved").dump(),
            "{\"ok\":true,\"x\":4,\"y\":4}"
        );
        assert_eq!(first_spy.moves.borrow().len(), 0);
        assert_eq!(second_spy.moves.borrow().as_slice(), &[(4, 4)]);
    }

    #[test]
    fn manual_probe_window_records_what_it_was_asked_to_do() {
        let mut window = ManualProbeWindow::default();
        assert_eq!((window.x(), window.y()), (0, 0));
        window.move_to(-3, 7);
        window.move_to(1, 1);
        assert_eq!(window.moves.as_slice(), &[(-3, 7), (1, 1)]);
        assert_eq!((window.x(), window.y()), (1, 1));
    }

    #[test]
    fn require_window_reports_the_python_error_text() {
        let mut bridge = PetProbeDragBridge::new(None);
        match bridge.require_window() {
            Ok(_) => panic!("an unbound bridge has no window"),
            Err(error) => {
                assert_eq!(error, ProbeError(ERROR_WINDOW_UNAVAILABLE));
                assert_eq!(error.to_string(), "pet_probe_window_unavailable");
            }
        }
    }

    // --------------------------------------------------- create_probe_window

    #[test]
    fn create_probe_window_refuses_on_windows() {
        let error = create_probe_window(&WebviewModule::pywebview(), Some("win32"))
            .expect_err("the signature alone is not a green light");
        assert_eq!(error, ProbeError(ERROR_CAPABILITY_UNAVAILABLE));
        assert_eq!(error.to_string(), "native_pet_window_capability_unavailable");
    }

    #[test]
    fn create_probe_window_refuses_an_unavailable_module() {
        let module = WebviewModule {
            has_create_window: false,
            has_start: false,
            accepts_var_keyword: false,
            parameters: vec![],
            signature_error: false,
        };
        for platform in [Some("darwin"), Some("win32"), None] {
            assert_eq!(
                create_probe_window(&module, platform).expect_err("unavailable"),
                ProbeError(ERROR_CAPABILITY_UNAVAILABLE)
            );
        }
    }

    #[test]
    fn create_probe_window_passes_the_python_keyword_set() {
        let probe = create_probe_window(&full_module(), Some("darwin")).expect("native window");
        assert_eq!(probe.spec.title, "ReadMD");
        assert_eq!(probe.spec.width, 220);
        assert_eq!(probe.spec.height, 240);
        assert_eq!(probe.spec.min_size, (160, 160));
        assert!(probe.spec.frameless && probe.spec.on_top && probe.spec.transparent);
        assert!(!probe.spec.text_select && !probe.spec.zoomable && !probe.spec.draggable && !probe.spec.shadow);
        assert_eq!(probe.spec.background_color, "#000000");
        assert_eq!(probe.spec.html, probe_html());
        // `js_api=bridge` plus `setattr(window, "_pet_probe_bridge", bridge)`:
        // the bridge is bound to the window the probe hands back.
        assert!(probe.bridge.is_bound());
    }

    // ------------------------------------------------------------ probe_html

    #[test]
    fn probe_html_is_the_authority_string_byte_for_byte() {
        // Digest, length and line count taken from the literal in
        // `window_adapter.py` itself by `scratch/rust_parity/_pet_launcher_s1_probe9.py`.
        let html = probe_html();
        assert_eq!(html.len(), 1208);
        assert_eq!(html.as_bytes().len(), 1208, "the literal is pure ASCII");
        assert_eq!(
            sha256_hex(html.as_bytes()),
            "dc59d846bc649a90347c4cf212179c66072d023b13226c3204d967924f4b7d80"
        );
        assert_eq!(html.split('\n').count(), 12);
        assert!(html.starts_with("<!doctype html><meta charset=\"utf-8\"><style>"));
        assert!(html.ends_with("</script>"));
        assert_eq!(html, html.trim(), "no padding around the literal");
    }

    #[test]
    fn probe_html_wires_the_three_bridge_methods_and_the_drag_class() {
        let html = probe_html();
        for needle in [
            "begin_drag",
            "move_drag",
            "end_drag",
            "window.pywebview&&window.pywebview.api",
            "e.screenX,e.screenY",
            "pointerdown",
            "pointermove",
            "pointerup",
            "pointercancel",
            "setPointerCapture",
            "releasePointerCapture",
            "classList.add('dragging')",
            "classList.remove('dragging')",
            "id=\"probe\"",
            "aria-label=\"ReadMD native pet capability probe\"",
            "background:transparent",
            "user-select:none",
            "cursor:grab",
            "#probe.dragging{cursor:grabbing}",
        ] {
            assert!(html.contains(needle), "{needle}");
        }
        assert!(!html.contains("Live2D"), "the probe is not a character");
    }

    // -------------------------------------------------------- probe_exit_code

    #[test]
    fn probe_exit_code_follows_only_native_window() {
        assert_eq!(probe_exit_code(&report("darwin", true, true, true, "x")), 0);
        assert_eq!(probe_exit_code(&report("win32", false, false, true, "x")), 2);
        assert_eq!(
            probe_exit_code(&OrdValue::object(vec![])),
            2,
            "`report[\"native_window\"]` missing means probe.py never gets that far"
        );
    }

    #[test]
    fn probe_exit_code_is_two_for_every_real_windows_report() {
        for module in [WebviewModule::pywebview(), full_module()] {
            assert_eq!(probe_exit_code(&probe_capabilities(&module, Some("win32"))), 2);
        }
        assert_eq!(probe_exit_code(&probe_capabilities(&full_module(), Some("darwin"))), 0);
    }

    // ---------------------------------------------------------- rounded_screen

    #[test]
    fn rounded_screen_matches_int_round_float() {
        for (value, want) in [
            (json!(0), 0i64),
            (json!(0.5), 0),
            (json!(1.5), 2),
            (json!(2.5), 2),
            (json!(3.5), 4),
            (json!(-0.5), 0),
            (json!(-1.5), -2),
            (json!(-2.5), -2),
            (json!("7"), 7),
            (json!(true), 1),
            (json!(false), 0),
        ] {
            assert_eq!(rounded_screen(&value).expect("number"), want, "{value}");
        }
        // Python's `int()` is unbounded and Rust's `i64` is not: a screen
        // coordinate beyond `i64::MAX` saturates instead of overflowing.  Safe
        // because `_safe_bounds`/`clamp` shrinks the value right after.
        assert!(rounded_screen(&json!("1e999")).is_err(), "infinite still raises");
    }
}
