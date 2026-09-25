//! ReadMD native kernel entry point.
//!
//! Boots the store, binds the HTTP kernel, then either opens a native webview
//! window or parks as a headless server for an external browser.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use readmd_kernel::paths::AppPaths;
use readmd_kernel::server;
use readmd_kernel::App;

// Replication lane `port-registry-ffi-s14`: the Win32 registry / shell FFI that
// replaced `install_association()`'s eight `reg add` children, its
// `ie4uinit.exe -show`, and the WebView2 `reg query`.  The module lives in
// `src/win_registry.rs`.
//
// `lib.rs` is held by the orchestrator, who adds the `pub mod win_registry;`
// declaration there; until that line lands the bin declares the module itself,
// so the FFI and its unit tests compile and run in this target.  The cleanup
// once it lands is one line: delete the declaration below and add a
// `use readmd_kernel::win_registry;` here instead, since no call site in this
// file names the crate at all.
//
// The `allow` is for this copy only: `query_string` and `delete_value` are part
// of the module's surface for a later uninstall path, not dead code in the lib.
#[allow(dead_code)]
#[path = "win_registry.rs"]
mod win_registry;

#[cfg(target_os = "windows")]
fn attach_console() {
    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(dwProcessId: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

const USAGE: &str = "usage: readmd [options] [FILE]

positional:
  FILE                markdown file to open at boot (sent as ?file=)

python options (readmd.py:6394-6418):
  --browser                 open the system browser and keep serving
  --port <port>             listen port; 0 (default) = control port 26891
                            with an ephemeral fallback
  --host <addr>             bind address, default 127.0.0.1
  --selftest                run the kernel self test and exit
  --webview-selftest        run the native webview network guard test and exit
  --mods                    report every extension module state and exit
  --share                   enable the LAN share right after boot
  --assoc                   register the .md open-with association and exit
  --startup-probe           record boot milestones, exit when the page is ready
  --startup-probe-json <p>  write the probe report to <p> (needs --startup-probe)
  --startup-probe-timeout <s>  probe timeout seconds, default 20
  --check-linux             diagnose Linux / Kylin / UOS and the GUI engine
  --check-windows           diagnose Windows and the Edge WebView2 runtime
  --check-macos             diagnose macOS and Cocoa WKWebView
  --diagnose, --check-system  unified native environment / graphics diagnosis

kernel-only (no readmd.py equivalent):
  --no-window             serve only, open neither window nor browser
  --data-dir <dir>        settings and database location
  --workspace <dir>       markdown root opened by the kernel
  --assets <dir>          frontend assets directory
  --require-token         require the instance token on /api calls
  --print-token           include the instance token in the boot banner
  -h, --help              show this help
  -V, --version           print version

option shapes (same rules as python argparse):
  --name=value            every value option also accepts the `=` form
  --prefix                long options may be abbreviated to any unambiguous
                          prefix (`--dia` = --diagnose); an ambiguous prefix is
                          reported with every candidate it matches
  --                      ends option scanning; later tokens are positional
";

/// Which `--check-*` diagnostic was requested (`readmd.py:6411-6418`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Check {
    Linux,
    Windows,
    MacOS,
    Diagnose,
}

/// One field per `readmd.py:6394-6418` `add_argument`; the kernel-only extras
/// are labelled in [`USAGE`].
struct Options {
    host: String,
    /// `parser.add_argument('--port', type=int, default=0)` (`readmd.py:6396`):
    /// a Python `int`, not a `u16`.  Anything outside the representable port
    /// range is *accepted* by `argparse` and only fails at bind time, where
    /// `start_server` falls back to an ephemeral port (`readmd.py:3694-3700`).
    port: i64,
    data_dir: Option<PathBuf>,
    workspace: Option<PathBuf>,
    assets: Option<PathBuf>,
    /// `parser.add_argument('file', nargs='?', help='要打开的 .md 文件')`
    /// (`readmd.py:6394`) — a document, never a workspace root.
    file: Option<String>,
    window: bool,
    browser: bool,
    share: bool,
    assoc: bool,
    selftest: bool,
    webview_selftest: bool,
    mods: bool,
    startup_probe: bool,
    startup_probe_json: Option<String>,
    startup_probe_timeout: f64,
    check: Option<Check>,
    print_token: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            host: "127.0.0.1".to_string(),
            port: 0,
            data_dir: None,
            workspace: None,
            assets: None,
            file: None,
            window: true,
            browser: false,
            share: false,
            assoc: false,
            selftest: false,
            webview_selftest: false,
            mods: false,
            startup_probe: false,
            startup_probe_json: None,
            // `--startup-probe-timeout', type=float, default=20.0`
            startup_probe_timeout: 20.0,
            check: None,
            print_token: false,
        }
    }
}

/// One action of `argparse`'s parser: every option string it registers, whether
/// it consumes a value, and the `dest` the kernel switches on.
struct Action {
    /// `action.option_strings`.  `--diagnose`/`--check-system` is one action with
    /// two strings, which is why Python names it `--diagnose/--check-system` in
    /// every error.
    strings: &'static [&'static str],
    takes_value: bool,
    dest: &'static str,
}

impl Action {
    /// `argparse._get_action_name()`: `'/'.join(action.option_strings)`.  That is
    /// the string every `argument …:` message prints.
    fn label(&self) -> String {
        self.strings.join("/")
    }
}

/// The *parity-visible* option table: exactly the actions `readmd.py:6393-6418`
/// registers, and nothing else.  This is the only table the abbreviation scan
/// reads, because `argparse._get_option_tuples()` scans
/// `self._option_string_actions` of *that* parser and a kernel-only invention is
/// not in it.
///
/// Order is CPython's `_option_string_actions` insertion order, not
/// `readmd.py`'s source order: `ArgumentParser.__init__` adds `-h/--help` before
/// `main()` gets to call `add_argument()`, so `--help` is the first key.  The
/// `ambiguous option:` message lists candidates in that order, which is directly
/// observable (`--h` → `could match --help, --host`, never `--host, --help`).
const PARITY: &[Action] = &[
    Action {
        strings: &["-h", "--help"],
        takes_value: false,
        dest: "help",
    },
    Action {
        strings: &["--browser"],
        takes_value: false,
        dest: "browser",
    },
    Action {
        strings: &["--port"],
        takes_value: true,
        dest: "port",
    },
    Action {
        strings: &["--host"],
        takes_value: true,
        dest: "host",
    },
    Action {
        strings: &["--selftest"],
        takes_value: false,
        dest: "selftest",
    },
    Action {
        strings: &["--webview-selftest"],
        takes_value: false,
        dest: "webview-selftest",
    },
    Action {
        strings: &["--mods"],
        takes_value: false,
        dest: "mods",
    },
    Action {
        strings: &["--share"],
        takes_value: false,
        dest: "share",
    },
    Action {
        strings: &["--assoc"],
        takes_value: false,
        dest: "assoc",
    },
    Action {
        strings: &["--startup-probe"],
        takes_value: false,
        dest: "startup-probe",
    },
    Action {
        strings: &["--startup-probe-json"],
        takes_value: true,
        dest: "startup-probe-json",
    },
    Action {
        strings: &["--startup-probe-timeout"],
        takes_value: true,
        dest: "startup-probe-timeout",
    },
    Action {
        strings: &["--check-linux"],
        takes_value: false,
        dest: "check-linux",
    },
    Action {
        strings: &["--check-windows"],
        takes_value: false,
        dest: "check-windows",
    },
    Action {
        strings: &["--check-macos"],
        takes_value: false,
        dest: "check-macos",
    },
    Action {
        strings: &["--diagnose", "--check-system"],
        takes_value: false,
        dest: "diagnose",
    },
];

/// The kernel-only switches the Rust binary needs to be driven headless but
/// which `readmd.py` never declared.  They are kept in their own table so they
/// are reachable by *exact spelling only* and are structurally invisible to the
/// abbreviation scan: otherwise `--p` matched `port` + `print-token` and turned
/// a prefix Python resolves uniquely into `ambiguous option` (exit 2).  A
/// collision between the two tables is impossible by construction because the
/// prefix scan only reads [`PARITY`], but the lookup order (parity exact →
/// parity prefix → kernel exact) still lets Python win if one is ever added.
const KERNEL_ONLY: &[Action] = &[
    Action {
        strings: &["-V", "--version"],
        takes_value: false,
        dest: "version",
    },
    Action {
        strings: &["--no-window"],
        takes_value: false,
        dest: "no-window",
    },
    Action {
        strings: &["--data-dir"],
        takes_value: true,
        dest: "data-dir",
    },
    Action {
        strings: &["--workspace", "--workspace-dir"],
        takes_value: true,
        dest: "workspace",
    },
    Action {
        strings: &["--assets", "--assets-dir"],
        takes_value: true,
        dest: "assets",
    },
    Action {
        strings: &["--require-token"],
        takes_value: false,
        dest: "require-token",
    },
    Action {
        strings: &["--print-token"],
        takes_value: false,
        dest: "print-token",
    },
];

fn find_exact(table: &'static [Action], token: &str) -> Option<(&'static str, &'static Action)> {
    for action in table {
        for string in action.strings {
            if *string == token {
                return Some((string, action));
            }
        }
    }
    None
}

/// `argparse`'s `_negative_number_matcher` (`^-\d+$|^-\d*\.\d+$`).  The parser
/// registers no option that looks like a negative number, so `_parse_optional`
/// lets such a token be consumed as the *value* of the option in front of it —
/// that is why `readmd.py --port -1` is a value and not an unknown flag — and,
/// with nothing in front of it, as the positional `file`: `readmd.py -5` boots
/// with `args.file == '-5'`.
fn looks_like_negative_number(token: &str) -> bool {
    let digits = match token.strip_prefix('-') {
        Some(rest) => rest,
        None => return false,
    };
    if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    // `-\d*\.\d+`: optional leading digits, one `.`, then at least one digit.
    match digits.split_once('.') {
        Some((head, tail)) => {
            (head.is_empty() || head.chars().all(|c| c.is_ascii_digit()))
                && !tail.is_empty()
                && tail.chars().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

fn parse_args() -> Result<Options, String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match parse_commandline(&argv) {
        Ok(Parsed::Options(opts)) => Ok(opts),
        // `argparse`'s help action writes to *stdout* and exits 0; anything after
        // it is never looked at.
        Ok(Parsed::Help) => {
            print!("{}", USAGE);
            exit(0);
        }
        Ok(Parsed::Version) => {
            println!("readmd-rust {}", server::VERSION);
            exit(0);
        }
        Err(err) => Err(err),
    }
}

/// What `argparse._parse_optional()` makes of one argv token.
enum Classified {
    /// It returned `None`: the token is a positional — and is therefore also
    /// legal as the value of the option in front of it.
    Positional,
    /// A matched action, the option string it resolved to, and the inline value
    /// of the `--name=value` form.
    Option(&'static Action, &'static str, Option<String>),
    /// It returned `(None, arg_string, None)`: `argparse` parks the token in
    /// `extras` and reports all of them once the scan is over.
    Unrecognized,
    /// The `--` sentinel.  Not an output of `_parse_optional` at all:
    /// `_parse_known_args()` matches it in its own pre-pass, marks every token
    /// behind it `'A'` without classifying them, and gives the sentinel the
    /// pattern character `'-'`, which no `nargs` pattern can turn into a value.
    Sentinel,
}

/// Port of `argparse.ArgumentParser._parse_optional`.  The step *order* is
/// CPython's and it is load bearing: the abbreviation scan runs before the
/// negative-number matcher and before the space rule, and it only ever reads
/// [`PARITY`].
fn classify(token: &str) -> Result<Classified, String> {
    // (1) an empty token, and (2) any token that does not start with a prefix
    // character, is positional.
    if token.is_empty() || !token.starts_with('-') {
        return Ok(Classified::Positional);
    }
    // (3) an exact option string wins.
    if let Some((string, action)) = find_exact(PARITY, token) {
        return Ok(Classified::Option(action, string, None));
    }
    // (4) a lone prefix character is positional.
    if token.chars().count() == 1 {
        return Ok(Classified::Positional);
    }
    let eq = token.find('=');
    // (5) `--name=value`, split on the FIRST `=`, when the left half is one of
    // the registered strings.
    if let Some(at) = eq {
        let head = &token[..at];
        if let Some((string, action)) = find_exact(PARITY, head) {
            return Ok(Classified::Option(
                action,
                string,
                Some(token[at + 1..].to_string()),
            ));
        }
    }
    // (6) the abbreviation scan.  `_get_option_tuples()` only splits a token that
    // starts with two prefix characters at `=`, and it only ever reads this
    // parser's own option strings.
    if token.starts_with("--") {
        let prefix = match eq {
            Some(at) => &token[..at],
            None => token,
        };
        let mut hits: Vec<(&str, &Action)> = Vec::new();
        for action in PARITY {
            for string in action.strings {
                if string.starts_with(prefix) {
                    hits.push((string, action));
                }
            }
        }
        match hits.len() {
            0 => {}
            1 => {
                return Ok(Classified::Option(
                    hits[0].1,
                    hits[0].0,
                    eq.map(|at| token[at + 1..].to_string()),
                ))
            }
            _ => {
                // `argparse` interpolates the whole argv token, not the part
                // before the `=`: `--h=x` → `ambiguous option: --h=x …`.
                let listed: Vec<String> = hits.iter().map(|(s, _)| s.to_string()).collect();
                return Err(format!(
                    "ambiguous option: {} could match {}",
                    token,
                    listed.join(", ")
                ));
            }
        }
    } else if eq.is_none() {
        // Single dash, no `=`: `-hx` reaches `-h` with an empty separator, so
        // `argparse` reads the tail as further short flags and `-h` still fires.
        // Measured: `-hh`, `-hx` and `-hV` all print help and exit 0, while
        // `-h=1` goes through step (5) and errors.  `-h` is the only single-dash
        // action this parser registers.
        let head: String = token.chars().take(2).collect();
        if let Some((string, action)) = find_exact(PARITY, &head) {
            return Ok(Classified::Option(action, string, None));
        }
    }
    // (6b) W4-09: the kernel-only escapes live in a *separate* match set that is
    // consulted here — after the Python-equivalent set has produced neither a
    // match nor an ambiguity — and by exact spelling only.  `_get_option_tuples()`
    // scans `self._option_string_actions` of the parser `readmd.py:6393-6418`
    // builds, and that dict holds no `--print-token`, `--data-dir` or
    // `--version`; the nine kernel-only spellings are therefore invisible to the
    // scan, which is what keeps `--p` = `--port` instead of
    // `ambiguous option: --p could match --port, --print-token` and `--print` an
    // unrecognized extra instead of a silently selected kernel flag.  Exact
    // spellings may not jump the queue either: a token that is both an exact
    // kernel-only string and an ambiguous parity prefix has to produce the
    // ambiguity `readmd.py` would report, which is why this block sits below the
    // scan instead of above it.
    if let Some((string, action)) = find_exact(KERNEL_ONLY, token) {
        return Ok(Classified::Option(action, string, None));
    }
    if let Some(at) = eq {
        let head = &token[..at];
        if let Some((string, action)) = find_exact(KERNEL_ONLY, head) {
            return Ok(Classified::Option(
                action,
                string,
                Some(token[at + 1..].to_string()),
            ));
        }
    }
    // (7) a negative-number-looking token is positional (`-5`, `-1.5`, `-.5`).
    if looks_like_negative_number(token) {
        return Ok(Classified::Positional);
    }
    // (8) a token containing a space is positional (`- 5`).
    if token.contains(' ') {
        return Ok(Classified::Positional);
    }
    // (9) otherwise it is an extra.
    Ok(Classified::Unrecognized)
}

/// `repr(str)` as CPython renders it in `argparse` messages: single quotes
/// unless the value contains a `'` and no `"`, `\n`/`\r`/`\t` by name, other
/// C0/C1 controls as `\xNN`, printable non-ASCII kept literally.
fn py_repr(value: &str) -> String {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(value.len() + 2);
    out.push(quote);
    for c in value.chars() {
        let code = c as u32;
        if c == '\\' {
            out.push_str("\\\\");
        } else if c == '\n' {
            out.push_str("\\n");
        } else if c == '\r' {
            out.push_str("\\r");
        } else if c == '\t' {
            out.push_str("\\t");
        } else if c == quote {
            out.push('\\');
            out.push(c);
        } else if code < 0x20 || (0x7f..0xa0).contains(&code) {
            out.push_str(&format!("\\x{:02x}", code));
        } else {
            out.push(c);
        }
    }
    out.push(quote);
    out
}

/// True when `c` is a CPython numeric-conversion separator: `str.strip()`'s
/// C1 range U+001C..U+001F is *not* whitespace for `int()`/`float()` (measured:
/// `int('\x1c5')` raises) while U+00A0 is, which is exactly Rust's
/// `char::is_whitespace` / Unicode `White_Space`.
fn is_py_number_space(c: char) -> bool {
    c.is_whitespace()
}

/// PEP 515 underscores: legal only between two digits — `1_0` and `.5_5` are,
/// `1__0`, `_1`, `1_`, `1_.5`, `1e_1` and `6.02e_23` all raise.
fn underscores_are_legal(chars: &[char]) -> bool {
    for (index, c) in chars.iter().enumerate() {
        if *c == '_' {
            let before = index > 0 && chars[index - 1].is_ascii_digit();
            let after = chars.get(index + 1).is_some_and(|c| c.is_ascii_digit());
            if !(before && after) {
                return false;
            }
        }
    }
    true
}

/// Eat a run of ASCII digits, stepping over PEP 515 underscores that
/// `underscores_are_legal` has already vouched for, and report how many real
/// digits were consumed.  Measured: `float("1_0.5") == 10.5` — an underscore
/// is part of the digit run, not a terminator of it.
fn eat_digits(chars: &[char], cursor: &mut usize) -> usize {
    let mut count = 0usize;
    while let Some(c) = chars.get(*cursor) {
        if c.is_ascii_digit() {
            count += 1;
            *cursor += 1;
        } else if *c == '_' && count > 0 {
            *cursor += 1;
        } else {
            break;
        }
    }
    count
}

/// `int(value)` as `argparse` calls it for `--port`.  A Python `int` is
/// unbounded, so an out-of-range magnitude saturates instead of failing: the
/// bind that follows is what rejects it in `readmd.py`, and a saturated port
/// fails there exactly like the real one does.
fn py_int(raw: &str) -> Option<i64> {
    let text = raw.trim_matches(is_py_number_space);
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit() || c == '_') {
        return None;
    }
    let chars: Vec<char> = digits.chars().collect();
    if !underscores_are_legal(&chars) {
        return None;
    }
    let clean: String = chars.iter().filter(|c| **c != '_').collect();
    // Every character is already known to be a digit, so the only way this
    // parse fails is overflow past u64 — and Python's unbounded int accepts
    // that spelling, so saturate rather than reject.
    let magnitude: u64 = clean.parse().unwrap_or(u64::MAX);
    Some(if negative {
        match i64::try_from(magnitude) {
            Ok(exact) => -exact,
            // 2**63 and beyond: `-9223372036854775808` is the only magnitude
            // that fits, and Python's unbounded int keeps everything above it
            // equally unusable as a port.
            Err(_) => i64::MIN,
        }
    } else {
        magnitude.min(i64::MAX as u64) as i64
    })
}

/// `float(value)` as `argparse` calls it for `--startup-probe-timeout`: the
/// same strip/underscore rules as `int`, plus `.5`, `5.`, an exponent, and the
/// case-insensitive `inf`/`infinity`/`nan` names.
fn py_float(raw: &str) -> Option<f64> {
    let text = raw.trim_matches(is_py_number_space);
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let name = body.to_ascii_lowercase();
    if name == "inf" || name == "infinity" {
        return Some(if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }
    if name == "nan" {
        return Some(f64::NAN);
    }
    let chars: Vec<char> = body.chars().collect();
    if !underscores_are_legal(&chars) {
        return None;
    }
    // Walk the grammar: ( digits [ . digits ] | . digits ) [ eE [+|-] digits ]
    let mut cursor = 0usize;
    let mut mantissa_digits = eat_digits(&chars, &mut cursor);
    if chars.get(cursor).copied() == Some('.') {
        cursor += 1;
        mantissa_digits += eat_digits(&chars, &mut cursor);
    }
    if mantissa_digits == 0 {
        return None;
    }
    if chars
        .get(cursor)
        .copied()
        .is_some_and(|c| c == 'e' || c == 'E')
    {
        cursor += 1;
        if chars
            .get(cursor)
            .copied()
            .is_some_and(|c| c == '+' || c == '-')
        {
            cursor += 1;
        }
        if eat_digits(&chars, &mut cursor) == 0 {
            return None;
        }
    }
    if cursor != chars.len() {
        return None;
    }
    let clean: String = chars.iter().filter(|c| **c != '_').collect();
    // Rust's float parser wants a digit on both sides of the `.`.
    let normalised = if clean.starts_with('.') {
        format!("0{}", clean)
    } else if clean.ends_with('.') {
        format!("{}0", clean)
    } else {
        clean
    };
    let parsed = normalised.parse::<f64>().ok()?;
    Some(if negative { -parsed } else { parsed })
}

/// Outcome of a clean scan: the flags, or one of the two immediate exits
/// `argparse` performs during the scan itself.
enum Parsed {
    Options(Options),
    Help,
    Version,
}

/// `parser.parse_args()` for the argv of [`parse_args`], returning `argparse`'s
/// own message text for every failure (the caller renders `usage`, the
/// `readmd: error:` prefix and exit code 2, as `parser.error()` does).
fn parse_commandline(argv: &[String]) -> Result<Parsed, String> {
    let mut opts = Options::default();
    // `argparse` keeps every token it could not place and reports them in one
    // message at the *end* of the scan: `x y z` → `unrecognized arguments: y z`.
    let mut extras: Vec<String> = Vec::new();
    // `_parse_known_args()` classifies the whole argv into its `O`/`A`/`-`
    // pattern *before* it consumes a single token, and `_parse_optional()` calls
    // `error()` from inside that pre-pass.  So an ambiguous abbreviation at a
    // later index is reported ahead of any conversion or missing-value complaint
    // an earlier token would raise.  Measured: `readmd.py --port=abc --h` is
    // `ambiguous option: --h could match --help, --host`, never
    // `argument --port: invalid int value: 'abc'`.
    let mut kinds: Vec<Classified> = Vec::with_capacity(argv.len());
    let mut scan = 0usize;
    while scan < argv.len() {
        // The `--` sentinel is matched by the pre-pass itself, before
        // `_parse_optional`, so neither it nor anything behind it reaches the
        // abbreviation scan — which would call a bare `--` ambiguous with all
        // seventeen long options — or the negative-number matcher.  A *second*
        // `--` is one of the tokens behind it, i.e. a plain positional.
        if argv[scan] == "--" {
            kinds.push(Classified::Sentinel);
            kinds.extend((scan + 1..argv.len()).map(|_| Classified::Positional));
            break;
        }
        kinds.push(classify(&argv[scan])?);
        scan += 1;
    }
    let mut i = 0usize;
    while i < kinds.len() {
        let token = argv[i].clone();
        let classified = &kinds[i];
        i += 1;
        match classified {
            Classified::Sentinel => {}
            Classified::Positional => push_positional(&mut opts, &mut extras, token),
            Classified::Unrecognized => extras.push(token),
            Classified::Option(action, _string, explicit) => {
                // A no-value action that was handed `--name=value` fails first:
                // `--help=1` is `argument -h/--help: ignored explicit argument
                // '1'` at exit 2, not help at exit 0.
                if !action.takes_value {
                    if let Some(given) = explicit {
                        return Err(format!(
                            "argument {}: ignored explicit argument {}",
                            action.label(),
                            py_repr(given)
                        ));
                    }
                }
                // The two immediate exits fire where they are found, so
                // `-h --bogus` still exits 0 with help on stdout.
                if action.dest == "help" {
                    return Ok(Parsed::Help);
                }
                if action.dest == "version" {
                    return Ok(Parsed::Version);
                }
                let mut raw = String::new();
                if action.takes_value {
                    raw = match explicit.clone() {
                        Some(given) => given,
                        None => {
                            // Read from the pre-pass rather than re-classifying:
                            // `_get_nargs_pattern()` matches `(-*A-*)` against the
                            // pattern characters, so only a token that came out
                            // `'A'` can feed the option — `-h`, `--nope` and the
                            // `--` sentinel itself all leave it dry.  Measured:
                            // `--startup-probe-timeout -- abc` is
                            // `expected one argument`, not `invalid float value`.
                            let next_is_value = kinds
                                .get(i)
                                .is_some_and(|kind| matches!(kind, Classified::Positional));
                            if !next_is_value {
                                return Err(format!(
                                    "argument {}: expected one argument",
                                    action.label()
                                ));
                            }
                            let taken = argv[i].clone();
                            i += 1;
                            taken
                        }
                    };
                }
                match action.dest {
                    "host" => opts.host = raw,
                    "port" => {
                        // `type=int` is an arbitrary Python `int`, not a `u16`:
                        // `--port 99999` is accepted by `argparse` and only fails
                        // at bind time, where `start_server` falls back to an
                        // ephemeral port (`readmd.py:3694-3700`).
                        opts.port = py_int(&raw).ok_or_else(|| {
                            format!(
                                "argument {}: invalid int value: {}",
                                action.label(),
                                py_repr(&raw)
                            )
                        })?;
                    }
                    "browser" => opts.browser = true,
                    "selftest" => opts.selftest = true,
                    "webview-selftest" => opts.webview_selftest = true,
                    "mods" => opts.mods = true,
                    "share" => opts.share = true,
                    "assoc" => opts.assoc = true,
                    "startup-probe" => opts.startup_probe = true,
                    "startup-probe-json" => opts.startup_probe_json = Some(raw),
                    "startup-probe-timeout" => {
                        opts.startup_probe_timeout = py_float(&raw).ok_or_else(|| {
                            format!(
                                "argument {}: invalid float value: {}",
                                action.label(),
                                py_repr(&raw)
                            )
                        })?;
                    }
                    "check-linux" => opts.check = Some(Check::Linux),
                    "check-windows" => opts.check = Some(Check::Windows),
                    "check-macos" => opts.check = Some(Check::MacOS),
                    "diagnose" => opts.check = Some(Check::Diagnose),
                    // Kernel-only (`readmd.py` has no equivalent at all).
                    "no-window" => opts.window = false,
                    "data-dir" => opts.data_dir = Some(PathBuf::from(raw)),
                    "workspace" => opts.workspace = Some(PathBuf::from(raw)),
                    "assets" => opts.assets = Some(PathBuf::from(raw)),
                    "require-token" => {
                        // `server::requires_token()` reads the env var, and the
                        // secret it validates is the running instance's
                        // `app.app_token`.
                        std::env::set_var("READMD_REQUIRE_TOKEN", "1");
                    }
                    "print-token" => opts.print_token = true,
                    other => return Err(format!("unrecognized arguments: --{}", other)),
                }
            }
        }
    }
    if !extras.is_empty() {
        return Err(format!("unrecognized arguments: {}", extras.join(" ")));
    }
    // `readmd.py:6438-6443` — argparse reports these through `parser.error`,
    // which is exit code 2, and the three messages below are its literal text:
    // Python is the authority for user-visible strings, so they are not
    // translated.  `<= 0` is Python's comparison written literally, which a
    // `nan` timeout passes exactly as it passes there.
    // `if args.startup_probe_json and ...` is Python *truthiness*: an explicit
    // `--startup-probe-json ''` is an empty string, which is falsy, so it does
    // not trip the check — `Option::is_some()` would.
    // `readmd.py:6421-6436` returns 0 before the checks below run, so a check
    // flag never trips them: `--diagnose --startup-probe --browser` prints its
    // report and exits 0 in Python.
    if opts.check.is_none() {
        let probe_json = opts
            .startup_probe_json
            .as_deref()
            .is_some_and(|value| !value.is_empty());
        if probe_json && !opts.startup_probe {
            return Err("--startup-probe-json 需要 --startup-probe".to_string());
        }
        if opts.startup_probe && opts.browser {
            return Err("--startup-probe 不能与 --browser 同时使用".to_string());
        }
        if opts.startup_probe_timeout <= 0.0 {
            return Err("--startup-probe-timeout 必须大于 0".to_string());
        }
    }
    Ok(Parsed::Options(opts))
}

/// `parser.add_argument('file', nargs='?')`: one slot, so the first positional
/// fills it and every later one becomes an extra.
fn push_positional(opts: &mut Options, extras: &mut Vec<String>, token: String) {
    if opts.file.is_none() {
        opts.file = Some(token);
    } else {
        extras.push(token);
    }
}

/// Every expectation below was printed by the installed CPython (3.11.15,
/// `python` on PATH — `python3` on this box is a broken stub) driving a parser
/// built from the literal `add_argument` block at `readmd.py:6393-6418`.  A
/// vector that "boots" is `parse_args()` returning normally, i.e. exit code 0;
/// a vector that "fails" is `parser.error()`, i.e. exit code 2 with the message
/// text given here.  One `#[test]` per measured vector.
#[cfg(test)]
mod cli_tests {
    use super::*;

    fn argv(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    /// The flags Python would have booted with.
    fn boots(items: &[&str]) -> Options {
        match parse_commandline(&argv(items)) {
            Ok(Parsed::Options(opts)) => opts,
            Ok(Parsed::Help) => panic!("{:?}: python exits 0 on help, not a boot", items),
            Ok(Parsed::Version) => panic!("{:?}: unexpected version exit", items),
            Err(err) => panic!("{:?}: python boots, kernel said: {}", items, err),
        }
    }

    /// `parser.error()`'s message text (exit code 2).
    fn fails(items: &[&str]) -> String {
        match parse_commandline(&argv(items)) {
            Err(err) => err,
            Ok(Parsed::Options(opts)) => panic!(
                "{:?}: python errors, kernel booted file={:?}",
                items, opts.file
            ),
            Ok(Parsed::Help) => panic!("{:?}: python errors, kernel printed help", items),
            Ok(Parsed::Version) => panic!("{:?}: python errors, kernel printed version", items),
        }
    }

    fn prints_help(items: &[&str]) {
        assert!(
            matches!(parse_commandline(&argv(items)), Ok(Parsed::Help)),
            "{:?}: expected argparse help + exit 0",
            items
        );
    }

    fn prints_version(items: &[&str]) {
        assert!(
            matches!(parse_commandline(&argv(items)), Ok(Parsed::Version)),
            "{:?}: expected the kernel-only version exit",
            items
        );
    }

    // ---------------------------------------------------------------- W4-08
    // A negative-number-looking token is a positional, never `unrecognized`.

    #[test]
    fn w4_08_lone_negative_is_the_file() {
        // ['-5'] -> exit 0, args.file == '-5'
        assert_eq!(boots(&["-5"]).file.as_deref(), Some("-5"));
    }

    #[test]
    fn w4_08_negative_after_an_option_is_the_file() {
        // ['--port','8080','-5'] -> exit 0, port 8080, file '-5'
        let opts = boots(&["--port", "8080", "-5"]);
        assert_eq!((opts.port, opts.file.as_deref()), (8080, Some("-5")));
    }

    #[test]
    fn w4_08_negative_decimal_is_the_file() {
        // ['-5.5'] -> file '-5.5'
        assert_eq!(boots(&["-5.5"]).file.as_deref(), Some("-5.5"));
    }

    #[test]
    fn w4_08_negative_one_is_the_file() {
        // ['-1'] -> file '-1'
        assert_eq!(boots(&["-1"]).file.as_deref(), Some("-1"));
    }

    #[test]
    fn w4_08_negative_one_point_five_is_the_file() {
        // ['-1.5'] -> file '-1.5'
        assert_eq!(boots(&["-1.5"]).file.as_deref(), Some("-1.5"));
    }

    #[test]
    fn w4_08_leading_dot_negative_is_the_file() {
        // ['-.5'] -> file '-.5' (`^-\d*\.\d+$` allows no leading digits)
        assert_eq!(boots(&["-.5"]).file.as_deref(), Some("-.5"));
    }

    #[test]
    fn w4_08_negative_zero_is_the_file() {
        // ['-0'] -> file '-0'
        assert_eq!(boots(&["-0"]).file.as_deref(), Some("-0"));
    }

    #[test]
    fn w4_08_after_end_of_options_marker() {
        // ['--','-5'] -> file '-5'
        assert_eq!(boots(&["--", "-5"]).file.as_deref(), Some("-5"));
    }

    #[test]
    fn w4_08_trailing_dot_is_not_a_number() {
        // ['-5.'] -> exit 2, unrecognized
        assert_eq!(fails(&["-5."]), "unrecognized arguments: -5.");
    }

    #[test]
    fn w4_08_double_dot_is_not_a_number() {
        // ['-5.5.5'] -> exit 2, unrecognized
        assert_eq!(fails(&["-5.5.5"]), "unrecognized arguments: -5.5.5");
    }

    #[test]
    fn w4_08_digit_then_letter_is_not_a_number() {
        // ['-5x'] -> exit 2, unrecognized
        assert_eq!(fails(&["-5x"]), "unrecognized arguments: -5x");
    }

    #[test]
    fn w4_08_underscore_is_not_a_number() {
        // ['-1x'] -> exit 2, unrecognized
        assert_eq!(fails(&["-1x"]), "unrecognized arguments: -1x");
    }

    #[test]
    fn w4_08_space_token_is_the_file() {
        // ['- 5'] -> file '- 5' (the `' ' in arg_string` rule)
        assert_eq!(boots(&["- 5"]).file.as_deref(), Some("- 5"));
    }

    #[test]
    fn w4_08_single_dash_is_the_file() {
        // ['-'] -> file '-'
        assert_eq!(boots(&["-"]).file.as_deref(), Some("-"));
    }

    #[test]
    fn w4_08_negative_after_a_filled_slot_is_an_extra() {
        // ['x','-5'] -> exit 2, unrecognized arguments: -5
        assert_eq!(fails(&["x", "-5"]), "unrecognized arguments: -5");
    }

    #[test]
    fn w4_08_two_negatives_only_the_first_is_the_file() {
        // ['-5','-6'] -> exit 2, unrecognized arguments: -6
        assert_eq!(fails(&["-5", "-6"]), "unrecognized arguments: -6");
    }

    #[test]
    fn w4_08_negative_still_fills_the_slot_behind_an_extra() {
        // ['--bogus','-5'] -> exit 2, only '--bogus' is the extra
        assert_eq!(fails(&["--bogus", "-5"]), "unrecognized arguments: --bogus");
    }

    #[test]
    fn w4_08_negative_before_an_extra() {
        // ['-5','--bogus'] -> exit 2, only '--bogus' is the extra
        assert_eq!(fails(&["-5", "--bogus"]), "unrecognized arguments: --bogus");
    }

    #[test]
    fn w4_08_negative_then_a_document_blames_the_document() {
        // The shape from the brief, `readmd -5 file.md`.  `-5` matches
        // `_negative_number_matcher`, so it is *not* an unknown optional: it
        // fills the `file` slot, and the document behind it is what
        // `unrecognized arguments` names.  Measured: rc 2,
        // "readmd.py: error: unrecognized arguments: x.md".
        assert_eq!(fails(&["-5", "x.md"]), "unrecognized arguments: x.md");
        assert_eq!(fails(&["-1.5", "x.md"]), "unrecognized arguments: x.md");
        assert_eq!(fails(&["-5.5", "x.md"]), "unrecognized arguments: x.md");
        assert_eq!(fails(&["-.5", "x.md"]), "unrecognized arguments: x.md");
        // The control half: shapes the matcher rejects come back as unknown
        // optionals, so they land in `extras` ahead of the document.  A third
        // positional then joins the extras in argv order.
        assert_eq!(fails(&["-5.", "x.md"]), "unrecognized arguments: -5.");
        assert_eq!(fails(&["-5x", "x.md"]), "unrecognized arguments: -5x");
        assert_eq!(
            fails(&["-5.", "x.md", "y.md"]),
            "unrecognized arguments: -5. y.md"
        );
    }

    #[test]
    fn w4_08_negative_value_then_positional() {
        // ['--port','-5','-6'] -> port -5, file '-6'
        let opts = boots(&["--port", "-5", "-6"]);
        assert_eq!((opts.port, opts.file.as_deref()), (-5, Some("-6")));
    }

    #[test]
    fn w4_08_negative_as_option_value() {
        // ['--port','-1'] -> port -1
        assert_eq!(boots(&["--port", "-1"]).port, -1);
    }

    #[test]
    fn w4_08_negative_string_as_host_value() {
        // ['--host','-5'] -> host '-5'
        assert_eq!(boots(&["--host", "-5"]).host, "-5");
    }

    #[test]
    fn w4_08_space_token_as_host_value() {
        // ['--host','- 5'] -> host '- 5'
        assert_eq!(boots(&["--host", "- 5"]).host, "- 5");
    }

    #[test]
    fn w4_08_bare_dash_is_consumed_as_a_value() {
        // ['--host','-'] -> host '-'
        assert_eq!(boots(&["--host", "-"]).host, "-");
    }

    #[test]
    fn w4_08_bare_dash_then_int_conversion_fails() {
        // ['--port','-'] -> exit 2, invalid int value: '-'
        assert_eq!(
            fails(&["--port", "-"]),
            "argument --port: invalid int value: '-'"
        );
    }

    #[test]
    fn w4_08_undashed_tail_is_not_a_number_value() {
        // ['--port','-1x'] -> exit 2, expected one argument
        assert_eq!(
            fails(&["--port", "-1x"]),
            "argument --port: expected one argument"
        );
    }

    #[test]
    fn w4_08_underscored_negative_is_not_a_number_value() {
        // ['--port','-1_0'] -> exit 2, expected one argument
        assert_eq!(
            fails(&["--port", "-1_0"]),
            "argument --port: expected one argument"
        );
    }

    #[test]
    fn w4_08_dashed_help_is_not_a_value() {
        // ['--port','-h'] -> exit 2, expected one argument
        assert_eq!(
            fails(&["--port", "-h"]),
            "argument --port: expected one argument"
        );
    }

    // ---------------------------------------------------------------- W4-09
    // The abbreviation scan sees readmd.py's options only.

    #[test]
    fn w4_09_p_is_port() {
        // ['--p','1'] -> port 1
        assert_eq!(boots(&["--p", "1"]).port, 1);
    }

    #[test]
    fn w4_09_p_without_a_value_is_ports_complaint() {
        // ['--p'] -> exit 2, 'argument --port: expected one argument'
        assert_eq!(fails(&["--p"]), "argument --port: expected one argument");
    }

    #[test]
    fn w4_09_p_equals_form() {
        // ['--p=1'] -> port 1
        assert_eq!(boots(&["--p=1"]).port, 1);
    }

    #[test]
    fn w4_09_a_is_assoc_not_assets() {
        // ['--a'] -> assoc True
        assert!(boots(&["--a"]).assoc);
    }

    #[test]
    fn w4_09_w_is_webview_selftest_not_workspace() {
        // ['--w'] -> webview_selftest True
        assert!(boots(&["--w"]).webview_selftest);
    }

    #[test]
    fn w4_09_d_is_diagnose_not_data_dir() {
        // ['--d'] -> diagnose True
        assert_eq!(boots(&["--d"]).check, Some(Check::Diagnose));
    }

    #[test]
    fn w4_09_v_is_unrecognized_like_python() {
        // ['--v'] -> exit 2, unrecognized (python has no --version at all)
        assert_eq!(fails(&["--v"]), "unrecognized arguments: --v");
    }

    #[test]
    fn w4_09_r_is_unrecognized_like_python() {
        // ['--r'] -> exit 2, unrecognized
        assert_eq!(fails(&["--r"]), "unrecognized arguments: --r");
    }

    #[test]
    fn w4_09_n_is_unrecognized_like_python() {
        // ['--n'] -> exit 2, unrecognized
        assert_eq!(fails(&["--n"]), "unrecognized arguments: --n");
    }

    #[test]
    fn w4_09_pr_is_unrecognized_like_python() {
        // ['--pr'] -> exit 2, unrecognized (only --port could ever match)
        assert_eq!(fails(&["--pr"]), "unrecognized arguments: --pr");
    }

    #[test]
    fn w4_09_kernel_flags_still_work_by_exact_spelling() {
        // Not python argv; the kernel keeps them out of band.
        assert!(boots(&["--print-token"]).print_token);
        assert!(boots(&["--require-token"]).window);
        assert!(!boots(&["--no-window"]).window);
        assert_eq!(
            boots(&["--data-dir", "d"]).data_dir,
            Some(PathBuf::from("d"))
        );
        assert_eq!(
            boots(&["--workspace", "w"]).workspace,
            Some(PathBuf::from("w"))
        );
        assert_eq!(
            boots(&["--workspace-dir", "w"]).workspace,
            Some(PathBuf::from("w"))
        );
        assert_eq!(boots(&["--assets", "a"]).assets, Some(PathBuf::from("a")));
        assert_eq!(
            boots(&["--assets-dir", "a"]).assets,
            Some(PathBuf::from("a"))
        );
        prints_version(&["--version"]);
        prints_version(&["-V"]);
    }

    #[test]
    fn w4_09_kernel_flags_are_not_abbreviatable() {
        assert_eq!(fails(&["--print"]), "unrecognized arguments: --print");
        assert_eq!(fails(&["--data-di"]), "unrecognized arguments: --data-di");
        assert_eq!(fails(&["--work"]), "unrecognized arguments: --work");
        assert_eq!(fails(&["--asse"]), "unrecognized arguments: --asse");
        assert_eq!(fails(&["--no"]), "unrecognized arguments: --no");
        assert_eq!(fails(&["--req"]), "unrecognized arguments: --req");
    }

    #[test]
    fn w4_09_kernel_flags_do_not_make_positionals_ambiguous() {
        // The kernel escapes are still accepted together, and their presence
        // does not turn any python abbreviation ambiguous.
        let opts = boots(&["--print-token", "--data-dir", "x"]);
        assert!(opts.print_token);
        assert_eq!(opts.data_dir, Some(PathBuf::from("x")));
        assert_eq!(boots(&["--print-token", "--p", "8080"]).port, 8080);
        assert!(boots(&["--assets", "a", "--a"]).assoc);
        assert_eq!(
            boots(&["--data-dir", "d", "--d"]).check,
            Some(Check::Diagnose)
        );
    }

    #[test]
    fn w4_09_help_first_in_the_ambiguity_list() {
        // ['--h'] -> 'ambiguous option: --h could match --help, --host'
        assert_eq!(
            fails(&["--h"]),
            "ambiguous option: --h could match --help, --host"
        );
    }

    #[test]
    fn w4_09_check_prefix_lists_declaration_order() {
        // ['--c'] -> the four --check*/--check-system strings, declared order
        assert_eq!(
            fails(&["--c"]),
            "ambiguous option: --c could match --check-linux, --check-windows, \
             --check-macos, --check-system"
        );
    }

    #[test]
    fn w4_09_check_word_is_ambiguous() {
        assert_eq!(
            fails(&["--check"]),
            "ambiguous option: --check could match --check-linux, --check-windows, \
             --check-macos, --check-system"
        );
    }

    #[test]
    fn w4_09_s_prefix_lists_five_startup_and_share_options() {
        assert_eq!(
            fails(&["--s"]),
            "ambiguous option: --s could match --selftest, --share, --startup-probe, \
             --startup-probe-json, --startup-probe-timeout"
        );
    }

    #[test]
    fn w4_09_st_prefix() {
        assert_eq!(
            fails(&["--st"]),
            "ambiguous option: --st could match --startup-probe, --startup-probe-json, \
             --startup-probe-timeout"
        );
    }

    #[test]
    fn w4_09_startup_pr_prefix() {
        assert_eq!(
            fails(&["--startup-pr"]),
            "ambiguous option: --startup-pr could match --startup-probe, \
             --startup-probe-json, --startup-probe-timeout"
        );
    }

    #[test]
    fn w4_09_bare_double_dash_equals_is_ambiguous_with_the_whole_token_quoted() {
        // ['--=x'] -> every long option, and the token printed as `--=x`
        assert_eq!(
            fails(&["--=x"]),
            "ambiguous option: --=x could match --help, --browser, --port, --host, \
             --selftest, --webview-selftest, --mods, --share, --assoc, --startup-probe, \
             --startup-probe-json, --startup-probe-timeout, --check-linux, \
             --check-windows, --check-macos, --diagnose, --check-system"
        );
    }

    #[test]
    fn w4_09_unique_prefixes_python_resolves() {
        assert_eq!(boots(&["--por", "80"]).port, 80);
        assert!(boots(&["--we"]).webview_selftest);
        assert!(boots(&["--web"]).webview_selftest);
        assert_eq!(boots(&["--dia"]).check, Some(Check::Diagnose));
        assert_eq!(boots(&["--check-s"]).check, Some(Check::Diagnose));
        assert_eq!(boots(&["--check-system"]).check, Some(Check::Diagnose));
        assert!(boots(&["--m"]).mods);
        assert!(boots(&["--b"]).browser);
        assert_eq!(
            boots(&["--startup-probe-t", "5"]).startup_probe_timeout,
            5.0
        );
        prints_help(&["--he"]);
    }

    // -------------------------------------------- messages, extras, precedence

    #[test]
    fn unrecognized_quotes_the_whole_token_including_the_value() {
        // ['--nope=1'] -> 'unrecognized arguments: --nope=1'
        assert_eq!(fails(&["--nope=1"]), "unrecognized arguments: --nope=1");
    }

    #[test]
    fn extras_are_aggregated_space_joined() {
        // ['x','y','z'] -> 'unrecognized arguments: y z'
        assert_eq!(fails(&["x", "y", "z"]), "unrecognized arguments: y z");
    }

    #[test]
    fn extras_aggregate_across_option_and_positional_kinds() {
        // ['--port','1','bad1','bad2','--nope'] -> 'bad2 --nope'
        assert_eq!(
            fails(&["--port", "1", "bad1", "bad2", "--nope"]),
            "unrecognized arguments: bad2 --nope"
        );
    }

    #[test]
    fn an_in_loop_option_error_beats_the_extras_list() {
        // ['a','b','--p'] -> --port complains, the extras list is never reached
        assert_eq!(
            fails(&["a", "b", "--p"]),
            "argument --port: expected one argument"
        );
    }

    #[test]
    fn ambiguity_is_reported_even_in_a_value_slot() {
        // ['--host','--c'] -> ambiguous, not 'expected one argument'
        assert_eq!(
            fails(&["--host", "--c"]),
            "ambiguous option: --c could match --check-linux, --check-windows, \
             --check-macos, --check-system"
        );
    }

    #[test]
    fn help_beats_later_extras_and_errors() {
        prints_help(&["--bogus", "-h"]);
        prints_help(&["-h", "--bogus"]);
    }

    #[test]
    fn double_dash_end_the_option_scan() {
        assert_eq!(boots(&["--", "--nope"]).file.as_deref(), Some("--nope"));
        assert_eq!(boots(&["--", "-h"]).file.as_deref(), Some("-h"));
        assert_eq!(boots(&["--", "-x"]).file.as_deref(), Some("-x"));
        // only the FIRST '--' is the sentinel: the second is a positional
        assert_eq!(fails(&["--", "-5", "--"]), "unrecognized arguments: --");
        assert_eq!(fails(&["--", "-5", "-6"]), "unrecognized arguments: -6");
    }

    #[test]
    fn double_dash_does_not_supply_a_value() {
        // ['--host','--'] -> exit 2, 'argument --host: expected one argument'
        assert_eq!(
            fails(&["--host", "--"]),
            "argument --host: expected one argument"
        );
        assert_eq!(
            fails(&["--host", "--", "x"]),
            "argument --host: expected one argument"
        );
    }

    #[test]
    fn value_options_take_the_last_occurrence() {
        assert_eq!(boots(&["--host", "a", "--host", "b"]).host, "b");
        assert_eq!(boots(&["--port", "1", "--port", "2"]).port, 2);
    }

    #[test]
    fn explicit_argument_errors_use_the_action_option_string_list() {
        // ['--d=1'] names both strings of the one action, exactly like python.
        assert_eq!(
            fails(&["--d=1"]),
            "argument --diagnose/--check-system: ignored explicit argument '1'"
        );
        assert_eq!(
            fails(&["--a=1"]),
            "argument --assoc: ignored explicit argument '1'"
        );
        assert_eq!(
            fails(&["--browser=x"]),
            "argument --browser: ignored explicit argument 'x'"
        );
    }

    #[test]
    fn check_flags_skip_the_startup_probe_conflicts() {
        // `readmd.py:6421-6436` returns 0 before the `parser.error()` calls at
        // 6438-6443, so the conflicts are only reachable without a diagnostic.
        assert_eq!(
            fails(&["--startup-probe", "--browser"]),
            "--startup-probe 不能与 --browser 同时使用"
        );
        assert_eq!(
            fails(&["--startup-probe-json", "probe.json"]),
            "--startup-probe-json 需要 --startup-probe"
        );
        assert_eq!(
            fails(&["--startup-probe-timeout", "0"]),
            "--startup-probe-timeout 必须大于 0"
        );
        for flag in [
            "--diagnose",
            "--check-system",
            "--check-linux",
            "--check-windows",
            "--check-macos",
        ] {
            let opts = boots(&[flag, "--startup-probe", "--browser"]);
            assert!(
                opts.startup_probe && opts.browser,
                "{flag}: the parser still sets both, only the exit differs"
            );
        }
        assert_eq!(boots(&["--diagnose"]).check, Some(Check::Diagnose));
        assert_eq!(boots(&["--check-system"]).check, Some(Check::Diagnose));
        assert_eq!(boots(&["--check-linux"]).check, Some(Check::Linux));
        assert_eq!(boots(&["--check-windows"]).check, Some(Check::Windows));
        assert_eq!(boots(&["--check-macos"]).check, Some(Check::MacOS));
    }

    #[test]
    fn help_with_an_explicit_value_is_not_help() {
        assert_eq!(
            fails(&["--help=1"]),
            "argument -h/--help: ignored explicit argument '1'"
        );
        assert_eq!(
            fails(&["-h=1"]),
            "argument -h/--help: ignored explicit argument '1'"
        );
    }

    #[test]
    fn short_flag_tail_still_fires_help() {
        // ['-hh'], ['-hx'], ['-hV'] all print help and exit 0.
        prints_help(&["-hh"]);
        prints_help(&["-hx"]);
        prints_help(&["-hV"]);
    }

    #[test]
    fn ambiguity_message_keeps_the_equals_tail() {
        assert_eq!(
            fails(&["--h=x"]),
            "ambiguous option: --h=x could match --help, --host"
        );
        assert_eq!(
            fails(&["--c=1"]),
            "ambiguous option: --c=1 could match --check-linux, --check-windows, \
             --check-macos, --check-system"
        );
    }

    #[test]
    fn odd_dash_tokens_follow_python() {
        assert_eq!(fails(&["---5"]), "unrecognized arguments: ---5");
        assert_eq!(fails(&["--1"]), "unrecognized arguments: --1");
        // ['-=-','x'] -> '-=-' is an extra, 'x' fills the positional
        assert_eq!(fails(&["-=-", "x"]), "unrecognized arguments: -=-");
        assert_eq!(boots(&["=x"]).file.as_deref(), Some("=x"));
    }

    #[test]
    fn py_repr_switches_to_double_quotes() {
        assert_eq!(py_repr("x"), "'x'");
        assert_eq!(py_repr("it's"), "\"it's\"");
        assert_eq!(py_repr("\"q\""), "'\"q\"'");
        assert_eq!(py_repr("a'b\"c"), "'a\\'b\"c'");
        assert_eq!(py_repr(""), "''");
        assert_eq!(py_repr("\u{7}"), "'\\x07'");
        assert_eq!(py_repr("é"), "'é'");
        assert_eq!(
            fails(&["--browser=it's"]),
            "argument --browser: ignored explicit argument \"it's\""
        );
    }

    #[test]
    fn empty_equals_value_is_distinct_from_absent() {
        // ['--host='] -> host '' (an empty string, not the default)
        let opts = boots(&["--host="]);
        assert_eq!(opts.host, "");
        assert_eq!(boots(&[]).host, "127.0.0.1");
        // ['--port='] -> invalid int value: ''
        assert_eq!(
            fails(&["--port="]),
            "argument --port: invalid int value: ''"
        );
        // ['--p=='] -> the value is the second '='
        assert_eq!(fails(&["--p=="]), "argument --port: invalid int value: '='");
    }

    #[test]
    fn startup_probe_json_uses_python_truthiness() {
        // An empty path is falsy, so readmd.py:6438 does not fire.
        assert_eq!(
            boots(&["--startup-probe-json", ""]).startup_probe_json,
            Some(String::new())
        );
        assert_eq!(
            boots(&["--startup-probe-json="]).startup_probe_json,
            Some(String::new())
        );
        assert_eq!(
            fails(&["--startup-probe-json", "x"]),
            "--startup-probe-json 需要 --startup-probe"
        );
    }

    #[test]
    fn post_parse_validations_run_after_the_extras_check() {
        // ['--startup-probe-json','x','zz1','zz2'] -> argparse's own message wins
        assert_eq!(
            fails(&["--startup-probe-json", "x", "zz1", "zz2"]),
            "unrecognized arguments: zz2"
        );
        assert_eq!(
            fails(&["--startup-probe", "--browser"]),
            "--startup-probe 不能与 --browser 同时使用"
        );
        assert_eq!(
            fails(&["--startup-probe-timeout", "0"]),
            "--startup-probe-timeout 必须大于 0"
        );
    }

    // ------------------------------------------------ int() / float() grammar

    #[test]
    fn py_int_matches_cpython() {
        let accepts: &[(&str, i64)] = &[
            ("5", 5),
            ("+5", 5),
            ("-5", -5),
            (" 5 ", 5),
            ("\t5\n", 5),
            ("1_0", 10),
            ("055", 55),
            ("0", 0),
        ];
        for (raw, want) in accepts {
            assert_eq!(py_int(raw), Some(*want), "int({:?})", raw);
        }
        for raw in [
            "", "-", "1.0", "0x10", "0b101", "1__0", "_1", "1_", "--5", "+-5", "1 2", "inf",
        ] {
            assert_eq!(py_int(raw), None, "int({:?}) must raise", raw);
        }
    }

    #[test]
    fn py_int_saturates_like_an_unbounded_python_int_then_fails_at_bind() {
        // ['--port','9999999999999999999999'] is valid argparse input; only the
        // bind rejects it.  Saturating keeps that exit-2-free.
        assert_eq!(boots(&["--port", "9999999999999999999999"]).port, i64::MAX);
        assert_eq!(boots(&["--port", "99999999999999999999"]).port, i64::MAX);
    }

    #[test]
    fn py_float_matches_cpython() {
        let accepts: &[(&str, f64)] = &[
            ("5", 5.0),
            ("1e3", 1000.0),
            (".5", 0.5),
            ("5.", 5.0),
            ("+.5", 0.5),
            ("1_0.5", 10.5),
            ("1_000.5e1", 10005.0),
            (".5_5", 0.55),
            ("  5  ", 5.0),
            ("-1", -1.0),
            ("055", 55.0),
            // Measured underscore placement: legal only between two digits.
            ("1_0e1_0", 1e11),
            ("1_0_0.0_0", 100.0),
            ("1e1_0", 1e10),
            ("1_0.0_1e1_0", 100100000000.0),
            ("1.e5", 100000.0),
            ("0_0", 0.0),
            ("-1_0.5", -10.5),
            ("+1_0e1_0", 1e11),
            ("1_000_000.5", 1000000.5),
        ];
        for (raw, want) in accepts {
            assert_eq!(py_float(raw), Some(*want), "float({:?})", raw);
        }
        for raw in [
            "", "-", ".", "1e", "5.5e", "e5", "1.5.5", "0x1p3", "1e_1", "6.02e_23", "1.5_", "_5",
            "1 2", "5,0", "1__0", "1._5", "1_.5", "1_.0", "_1.0", "1_e0", "1_0e_1", ".5_", "1.__0",
            "inf_", "._5", "1_", "__", "1_0.__1", "1.0e1__0",
        ] {
            assert_eq!(py_float(raw), None, "float({:?}) must raise", raw);
        }
        assert!(py_float("inf") == Some(f64::INFINITY));
        assert!(py_float("INFINITY") == Some(f64::INFINITY));
        assert!(py_float("Infinity") == Some(f64::INFINITY));
        assert!(py_float("-inf") == Some(f64::NEG_INFINITY));
        assert!(py_float("nan").map(f64::is_nan) == Some(true));
        assert!(py_float("-nan").map(f64::is_nan) == Some(true));
    }

    #[test]
    fn nan_timeout_passes_the_zero_check_exactly_like_python() {
        // `float('nan') <= 0` is False, so readmd.py boots.
        assert!(boots(&["--startup-probe-timeout", "nan"])
            .startup_probe_timeout
            .is_nan());
        assert_eq!(
            fails(&["--startup-probe-timeout", "-1"]),
            "--startup-probe-timeout 必须大于 0"
        );
    }

    #[test]
    fn non_ascii_digits_are_a_known_residual() {
        // CPython maps Unicode decimal digits: int('٥') == 5.  The kernel
        // rejects them; recorded rather than silently divergent.
        assert_eq!(py_int("\u{665}"), None);
    }

    #[test]
    fn negative_number_matcher_table() {
        for token in ["-5", "-1", "-0", "-5.5", "-.5", "-1.5", "-007", "-00.5"] {
            assert!(looks_like_negative_number(token), "{}", token);
        }
        for token in [
            "-", "-5.", "-5.5.5", "-5x", "-1_0", "--5", "+5", "5", "", "-a", "- 5",
        ] {
            assert!(!looks_like_negative_number(token), "{}", token);
        }
    }

    #[test]
    fn defaults_match_argparse_defaults() {
        let opts = boots(&[]);
        assert_eq!(opts.host, "127.0.0.1");
        assert_eq!(opts.port, 0);
        assert_eq!(opts.startup_probe_timeout, 20.0);
        assert_eq!(opts.file, None);
        assert_eq!(opts.startup_probe_json, None);
        assert!(opts.window);
        assert!(!opts.browser);
        assert!(!opts.share);
        assert!(!opts.assoc);
        assert!(!opts.selftest);
        assert!(!opts.webview_selftest);
        assert!(!opts.mods);
        assert!(!opts.startup_probe);
        assert!(!opts.print_token);
        assert_eq!(opts.check, None);
    }

    #[test]
    fn parity_table_is_exactly_readmd_py_own_strings() {
        // Anything the python parser does not declare must stay out of PARITY:
        // that table is what the abbreviation scan reads.
        let declared: Vec<&str> = PARITY
            .iter()
            .flat_map(|action| action.strings.iter().copied())
            .collect();
        assert_eq!(
            declared,
            vec![
                "-h",
                "--help",
                "--browser",
                "--port",
                "--host",
                "--selftest",
                "--webview-selftest",
                "--mods",
                "--share",
                "--assoc",
                "--startup-probe",
                "--startup-probe-json",
                "--startup-probe-timeout",
                "--check-linux",
                "--check-windows",
                "--check-macos",
                "--diagnose",
                "--check-system",
            ]
        );
        // No kernel-only invention is prefix-visible, and none of them collides
        // with a python option string.
        for action in KERNEL_ONLY {
            for string in action.strings {
                assert!(!declared.contains(&string), "{} leaks into PARITY", string);
                assert!(find_exact(PARITY, string).is_none());
            }
        }
    }

    #[test]
    fn numeric_whitespace_set_matches_cpython_conversion() {
        // Measured: int('\xa05') == 5 while int('\x1c5') raises, even though
        // '\x1c'.isspace() is True — `int()` does not use `str.strip()`'s set.
        assert_eq!(py_int("\u{a0}5"), Some(5));
        assert_eq!(py_int("\u{1c}5"), None);
        assert_eq!(py_int("5\u{1f}"), None);
        assert_eq!(py_float("\u{a0}5"), Some(5.0));
        assert_eq!(py_float("\u{1c}5"), None);
        assert_eq!(py_int(" \u{1c} 5 \u{1f} "), None);
    }

    // ------------------------------------------------- W4-09, the full sweep
    //
    // `PYTHON_LONG` is `readmd.py:6393-6418` plus the `-h/--help` action
    // `ArgumentParser.__init__` adds, spelled out a second time on purpose: the
    // enumeration below derives its expectations from this list, so a [`PARITY`]
    // table that drifted from `readmd.py` cannot agree with itself.  (The table's
    // own contents are pinned by `parity_table_is_exactly_readmd_py_own_strings`.)
    // Order is `_option_string_actions` insertion order, which CPython's
    // `ambiguous option:` message prints.
    const PYTHON_LONG: &[&str] = &[
        "--help",
        "--browser",
        "--port",
        "--host",
        "--selftest",
        "--webview-selftest",
        "--mods",
        "--share",
        "--assoc",
        "--startup-probe",
        "--startup-probe-json",
        "--startup-probe-timeout",
        "--check-linux",
        "--check-windows",
        "--check-macos",
        "--diagnose",
        "--check-system",
    ];

    /// What `classify()` settled on, as text.  `Classified` deliberately does not
    /// derive `Debug` (it carries a `&'static Action`), and the sweep below wants
    /// a one-line diff for a hundred and sixty shapes.
    fn described(token: &str) -> String {
        match classify(token) {
            Ok(Classified::Positional) => "positional".to_string(),
            Ok(Classified::Option(_, string, _)) => format!("option {}", string),
            Ok(Classified::Unrecognized) => format!("unrecognized {}", token),
            Ok(Classified::Sentinel) => "sentinel".to_string(),
            Err(message) => message,
        }
    }

    /// CPython's `_get_option_tuples()` for a `--` token, recomputed from
    /// [`PYTHON_LONG`] alone: every registered string that starts with the
    /// abbreviation, in insertion order.
    fn cpython_hits(prefix: &str) -> Vec<&'static str> {
        PYTHON_LONG
            .iter()
            .copied()
            .filter(|string| string.starts_with(prefix))
            .collect()
    }

    /// The verdict CPython's `_parse_optional()` returns for one token, given the
    /// candidates its scan found.  Exact membership is tested *first* (step 3),
    /// so a full spelling is never ambiguous with the longer options it happens
    /// to be a prefix of: `--startup-probe` is `--startup-probe`, not a three-way
    /// `ambiguous option`, and the scan only decides the abbreviations.
    fn cpython_verdict(token: &str, hits: &[&str]) -> String {
        if PYTHON_LONG.contains(&token) {
            return format!("option {}", token);
        }
        match hits.len() {
            0 => format!("unrecognized {}", token),
            1 => format!("option {}", hits[0]),
            _ => format!(
                "ambiguous option: {} could match {}",
                token,
                hits.join(", ")
            ),
        }
    }

    #[test]
    fn w4_09_every_python_long_option_and_abbreviation_agrees() {
        let mut shapes = 0usize;
        let mut unique = 0usize;
        let mut ambiguous = 0usize;
        for string in PYTHON_LONG {
            let chars: Vec<char> = string.chars().collect();
            // The full spelling, then every abbreviation CPython accepts — down
            // to three characters, because `--` alone is the sentinel and never
            // reaches `_parse_optional`.
            let mut forms: Vec<String> = vec![string.to_string()];
            forms.extend((3..chars.len()).map(|cut| chars[..cut].iter().collect::<String>()));
            for form in &forms {
                let hits = cpython_hits(form);
                if hits.len() == 1 {
                    unique += 1;
                } else if hits.len() > 1 {
                    ambiguous += 1;
                }
                shapes += 1;
                assert_eq!(
                    described(form),
                    cpython_verdict(form, &hits),
                    "{} is a python option/abbreviation",
                    form
                );
                // A kernel-only spelling must never be the answer to a python
                // abbreviation, whatever the candidate count is.
                if let Ok(Classified::Option(_, resolved, _)) = classify(form) {
                    assert!(
                        find_exact(PARITY, resolved).is_some(),
                        "{} resolved to the kernel-only {}",
                        form,
                        resolved
                    );
                }
            }
        }
        // Guards the sweep itself: a silent `forms` regression would otherwise
        // make it vacuously true.
        assert!(shapes > 150, "only {} shapes enumerated", shapes);
        assert!(unique > 40, "only {} unique resolutions", unique);
        assert!(ambiguous > 20, "only {} ambiguous shapes", ambiguous);
    }

    #[test]
    fn w4_09_no_kernel_only_option_is_reachable_by_an_abbreviation() {
        for action in KERNEL_ONLY {
            for string in action.strings {
                // Exact spelling is the only door into the second table.
                assert_eq!(
                    described(string),
                    format!("option {}", string),
                    "kernel-only {} must resolve by its exact spelling",
                    string
                );
                if !string.starts_with("--") {
                    continue;
                }
                let chars: Vec<char> = string.chars().collect();
                for cut in 3..chars.len() {
                    let form: String = chars[..cut].iter().collect();
                    // A proper prefix that is itself another kernel-only spelling
                    // (`--workspace`, a prefix of `--workspace-dir`) is exact-table
                    // input, not an abbreviation: the exact door still opens.
                    if find_exact(KERNEL_ONLY, form.as_str()).is_some() {
                        assert_eq!(
                            described(&form),
                            format!("option {}", form),
                            "{} is an exact kernel-only spelling",
                            form
                        );
                        continue;
                    }
                    let hits = cpython_hits(&form);
                    // What `readmd.py` says for that prefix — `--w` is
                    // `--webview-selftest`, `--a` is `--assoc` — and never the
                    // kernel action the prefix was cut out of.
                    assert_eq!(described(&form), cpython_verdict(&form, &hits));
                }
            }
        }
        // `-V` is single dash, so CPython's concatenated short-flag form would be
        // `-V5`.  The second table is exact-spelling only, so the tail is not a
        // version request but the unrecognized extra `readmd.py` reports for it.
        assert_eq!(described("-V5"), "unrecognized -V5");
        assert_eq!(described("-hV5"), "option -h");
    }

    #[test]
    fn the_prepass_error_beats_a_later_walk_error() {
        // Measured: `readmd.py --port=abc --h` is the ambiguity, because
        // `_parse_known_args()` classifies every token before it consumes one —
        // even though `--port`'s conversion would have failed on token zero.
        assert_eq!(
            fails(&["--port=abc", "--h"]),
            "ambiguous option: --h could match --help, --host"
        );
        assert_eq!(
            fails(&["--h", "--port"]),
            "ambiguous option: --h could match --help, --host"
        );
        assert_eq!(
            fails(&["--browser=x", "--h"]),
            "ambiguous option: --h could match --help, --host"
        );
        // `-x` classifies cleanly (it is an extra), so nothing jumps ahead of the
        // walk: `--port`'s own complaint is what `readmd.py` prints here.
        assert_eq!(
            fails(&["--port=abc", "-x"]),
            "argument --port: invalid int value: 'abc'"
        );
    }
}

fn resolve_paths(opts: &Options) -> readmd_kernel::Result<AppPaths> {
    let base = AppPaths::resolve()?;
    if opts.data_dir.is_none() && opts.workspace.is_none() && opts.assets.is_none() {
        return Ok(base);
    }
    let data = opts.data_dir.clone().unwrap_or_else(|| base.data_dir.clone());
    let workspace = opts.workspace.clone().unwrap_or_else(|| base.workspace.clone());
    let assets = opts.assets.clone().unwrap_or_else(|| base.assets_dir.clone());
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let data_abs = if data.is_absolute() { data } else { cwd.join(data) };
    let workspace_abs = if workspace.is_absolute() { workspace } else { cwd.join(workspace) };
    let assets_abs = if assets.is_absolute() { assets } else { cwd.join(assets) };
    let data = readmd_kernel::paths::canonicalize_or_clean(&data_abs);
    let workspace = readmd_kernel::paths::canonicalize_or_clean(&workspace_abs);
    let assets = readmd_kernel::paths::canonicalize_or_clean(&assets_abs);
    std::fs::create_dir_all(&data)?;
    // Only ever a directory here: the positional document is `opts.file`, so a
    // `.md` path can no longer be `create_dir_all`'d into a folder of its name.
    std::fs::create_dir_all(&workspace)?;
    Ok(AppPaths::with_dirs(&data, &workspace, &assets))
}

/// `os.path.abspath()` (`readmd.py:6490`, `readmd.py:6511`): make the path
/// absolute and lexically clean without requiring it to exist.
fn abspath(raw: &str) -> String {
    let candidate = PathBuf::from(raw);
    let joined = if candidate.is_absolute() {
        candidate
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(&candidate)
    };
    readmd_kernel::paths::canonicalize_or_clean(&joined)
        .to_string_lossy()
        .into_owned()
}

/// `readmd.py:6509-6515`: `os.path.abspath()` then keep the path only when
/// `os.path.isfile()` already holds; a missing target is a printed warning and
/// the reader boots with no initial document.
fn resolve_initial(file: Option<&String>) -> Option<String> {
    let raw = file?;
    let absolute = PathBuf::from(abspath(raw));
    if absolute.is_file() {
        return Some(absolute.to_string_lossy().into_owned());
    }
    // `readmd.py:6515` prints Python's own wording, not a translation:
    // `safe_print('文件不存在: %s' % args.file)`.
    println!("文件不存在: {}", raw);
    None
}

/// `_finish_startup_probe(timed_out=True)` (`readmd.py:270-288`) records the
/// timeout before destroying the window, so the report can tell a slow boot from
/// a fast one.
fn probe_set_timed_out(probe: &Probe) {
    probe.lock().unwrap_or_else(|e| e.into_inner()).timed_out = true;
}

/// The one tail every windowed exit passes through (`readmd.py:6691-6709`).
///
/// A probe run persists its report first, then `instance.json` is removed and the
/// exit code Python returns is handed back to the caller.  `done` guards the whole
/// thing because the tao event loop can reach the exit decision more than once and
/// printing the report twice would corrupt a `--startup-probe-json` consumer that
/// reads the last line.
fn shutdown(
    done: &AtomicBool,
    data_dir: &Path,
    probe: &Option<Probe>,
    probe_json: &Option<String>,
    timed_out: bool,
    failed: bool,
) -> i32 {
    if done.swap(true, Ordering::SeqCst) {
        return if failed { 1 } else { 0 };
    }
    // Python leaves the early `return 1` paths without a `_clear_instance()` call
    // even though `start_server` already published the entry; the kernel clears it
    // on every exit so a failed boot cannot strand a dead control address.
    let code = if failed { 1 } else { 0 };
    let Some(probe) = probe else {
        clear_instance(data_dir);
        return code;
    };
    if timed_out {
        probe_set_timed_out(probe);
    }
    let report = probe_report(probe);
    let wrote = probe_write(&report, probe_json.as_deref());
    clear_instance(data_dir);
    if !wrote {
        println!("startup probe write failed: {}", probe_json.clone().unwrap_or_default());
        return 1;
    }
    if timed_out || failed {
        1
    } else {
        0
    }
}

/// `urllib.parse.quote(initial)` as used by `readmd.py:6520`, i.e. the default
/// `safe='/'`: letters, digits, `_.-~` and `/` pass through, every other byte —
/// including each byte of a UTF-8 CJK path — becomes `%XX`.
fn url_quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-' | b'~' | b'/') {
            out.push(*byte as char);
        } else {
            out.push_str(&format!("%{:02X}", byte));
        }
    }
    out
}

// ------------------------------------------------------ single-instance probe
//
// `readmd.py:6486-6491`: a second launch pings the resident instance through
// the fixed `CONTROL_PORT` + `instance.json` pair, forwards the file it was
// handed and exits 0, which is what makes a double-clicked `.md` open instantly
// in the already-running window instead of spawning a second kernel.

fn instance_path(data_dir: &Path) -> PathBuf {
    data_dir.join("instance.json")
}

/// `_read_instance()` (`readmd.py:299`).
fn read_instance(data_dir: &Path) -> Option<(u16, String)> {
    let text = std::fs::read_to_string(instance_path(data_dir)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let port = value.get("port").and_then(|p| p.as_u64())? as u16;
    let token = value
        .get("token")
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .to_string();
    if port == 0 || token.is_empty() {
        return None;
    }
    Some((port, token))
}

/// `_clear_instance()` (`readmd.py:308-313`).
fn clear_instance(data_dir: &Path) {
    let file = instance_path(data_dir);
    if file.is_file() {
        let _ = std::fs::remove_file(&file);
    }
}

/// One-shot loopback HTTP/1.1 request against our own server.  Hand-rolled on
/// `std::net` so the kernel gains neither a Python nor an HTTP-client runtime
/// dependency for its own startup probe.  `headers` may override `Host` or add
/// `Origin` — the only way to exercise the loopback guard from the same process.
fn loopback_exchange(
    port: u16,
    method: &str,
    path: &str,
    body: Option<&str>,
    headers: &[(&str, &str)],
    timeout: Duration,
) -> Option<(u16, String)> {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    let authority = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("host"))
        .map(|(_, value)| (*value).to_string())
        .unwrap_or_else(|| format!("127.0.0.1:{}", port));
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    let payload = body.unwrap_or("");
    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n");
    if let Some(raw) = body {
        request.push_str("Content-Type: application/json\r\n");
        request.push_str(&format!("Content-Length: {}\r\n", raw.len()));
    }
    for (name, value) in headers {
        if !name.eq_ignore_ascii_case("host") {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    request.push_str("\r\n");
    request.push_str(payload);
    stream.write_all(request.as_bytes()).ok()?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).ok()?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let status_line = text.lines().next().unwrap_or_default().to_string();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .unwrap_or(0);
    match text.find("\r\n\r\n") {
        Some(split) => Some((status, text[split + 4..].to_string())),
        None => None,
    }
}

fn loopback_body(
    port: u16,
    method: &str,
    path: &str,
    body: Option<&str>,
    timeout: Duration,
) -> Option<String> {
    loopback_exchange(port, method, path, body, &[], timeout).map(|(_, text)| text)
}

fn json_field(body: &Option<String>, key: &str) -> Option<String> {
    body.as_deref()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
        .and_then(|value| {
            value.get(key).map(|field| match field {
                serde_json::Value::Bool(flag) => flag.to_string(),
                serde_json::Value::String(text) => text.clone(),
                serde_json::Value::Null => String::new(),
                other => other.to_string(),
            })
        })
}

fn body_is_ok(body: &Option<String>) -> bool {
    json_field(body, "ok").as_deref() == Some("true")
}

/// `_ping_instance(port, token, timeout=0.8)` (`readmd.py:316-326`).
fn ping_instance(port: u16, token: &str) -> bool {
    body_is_ok(&loopback_body(
        port,
        "GET",
        &format!("/api/ping?t={}", token),
        None,
        Duration::from_millis(800),
    ))
}

/// `instance_alive()` (`readmd.py:328-335`).
fn instance_alive(data_dir: &Path) -> Option<(u16, String)> {
    let (port, token) = read_instance(data_dir)?;
    if ping_instance(port, &token) {
        Some((port, token))
    } else {
        None
    }
}

/// `forward_open(port, token, path)` (`readmd.py:337-348`): POST the document to
/// `/api/control/open` of the resident instance; 3s timeout, `ok` required.
fn forward_open(port: u16, token: &str, path: &str) -> bool {
    let body = serde_json::json!({ "token": token, "file": path }).to_string();
    body_is_ok(&loopback_body(
        port,
        "POST",
        "/api/control/open",
        Some(&body),
        Duration::from_secs(3),
    ))
}

// ------------------------------------------------------------- process control

/// Set by the console/signal hook so a headless run can shut down cleanly
/// (`readmd.py:6528-6531` catches `KeyboardInterrupt`, then `_clear_instance()`).
static QUIT: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
fn install_quit_handler() {
    #[link(name = "kernel32")]
    extern "system" {
        fn SetConsoleCtrlHandler(
            handler: Option<unsafe extern "system" fn(u32) -> i32>,
            add: i32,
        ) -> i32;
    }
    // CTRL_C_EVENT 0, CTRL_BREAK_EVENT 1, CTRL_CLOSE_EVENT 2,
    // CTRL_LOGOFF_EVENT 5, CTRL_SHUTDOWN_EVENT 6.  Returning TRUE keeps the
    // default handler from killing us before `instance.json` is removed.
    unsafe extern "system" fn on_ctrl(ctrl_type: u32) -> i32 {
        if matches!(ctrl_type, 0 | 1 | 2 | 5 | 6) {
            QUIT.store(true, Ordering::SeqCst);
            return 1;
        }
        0
    }
    unsafe {
        SetConsoleCtrlHandler(Some(on_ctrl), 1);
    }
}

#[cfg(not(target_os = "windows"))]
fn install_quit_handler() {
    #[link(name = "c")]
    extern "C" {
        fn signal(signum: i32, handler: Option<unsafe extern "C" fn(i32)>) -> usize;
    }
    unsafe extern "C" fn on_signal(_signum: i32) {
        QUIT.store(true, Ordering::SeqCst);
    }
    // SIGINT / SIGTERM.
    unsafe {
        signal(2, Some(on_signal));
        signal(15, Some(on_signal));
    }
}

/// Park until the quit signal arrives, mirroring Python's
/// `while True: threading.Event().wait(3600)` inside a `try/except
/// KeyboardInterrupt`.  The old `sleep(600)` loop could not be interrupted.
fn park_until_quit() {
    install_quit_handler();
    while !QUIT.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(120));
    }
}

/// `webbrowser.open(url)` (`readmd.py:6523`): hand the URL to the OS default
/// handler.  Crate-free `ShellExecuteW` on Windows; `open` / `xdg-open`
/// elsewhere.  No temp browser profile is created, so nothing can leak.
fn open_in_default_browser(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = readmd_kernel::native_system::shell_open(url);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

/// Standalone browser-app window used only when the native webview cannot be
/// built — the Rust mirror of `windows_native.launch_browser_app(url)`, which
/// Python also only reaches from that same failure branch (`readmd.py:6658-6672`).
fn launch_browser_app(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let candidates = [
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ];
        let temp_profile = std::env::temp_dir().join(format!("readmd_app_{}", std::process::id()));
        for cand in &candidates {
            if !std::path::Path::new(cand).exists() {
                continue;
            }
            if std::fs::create_dir_all(&temp_profile).is_err() {
                break;
            }
            if let Ok(mut child) = std::process::Command::new(cand)
                .arg(format!("--app={}", url))
                .arg(format!("--user-data-dir={}", temp_profile.display()))
                .arg("--window-size=1160,820")
                .arg("--no-first-run")
                .arg("--no-default-browser-check")
                .spawn()
            {
                let _ = child.wait();
                let _ = std::fs::remove_dir_all(&temp_profile);
                return;
            }
        }
        // Every candidate path failed or was absent: never leave the profile
        // directory behind before dropping to the plain-browser fallback.
        let _ = std::fs::remove_dir_all(&temp_profile);
    }
    open_in_default_browser(url);
    park_until_quit();
}

// ------------------------------------------------------------- startup probe
//
// `--startup-probe` (`readmd.py:6451-6457`, `6562-6569`, `6591-6592`,
// `6696-6702`): record boot milestones, destroy the window once the page is
// ready (or the timeout fires), write the JSON report and exit non-zero when it
// had to time out.

struct ProbeInner {
    started: Instant,
    timeout: Duration,
    milestones: Vec<(&'static str, u64)>,
    timed_out: bool,
}

type Probe = Arc<Mutex<ProbeInner>>;

fn probe_mark(probe: &Option<Probe>, name: &'static str) {
    let Some(probe) = probe else { return };
    let mut state = probe.lock().unwrap_or_else(|e| e.into_inner());
    if state.milestones.iter().any(|(seen, _)| *seen == name) {
        return;
    }
    let elapsed = state.started.elapsed().as_millis() as u64;
    state.milestones.push((name, elapsed));
}

/// `startup_probe_summary()` (`readmd.py:244-252`).  Privacy-safe by
/// construction: only elapsed milliseconds and a timeout flag, never a path.
fn probe_report(probe: &Probe) -> serde_json::Value {
    let (timed_out, milestones) = {
        let state = probe.lock().unwrap_or_else(|e| e.into_inner());
        (state.timed_out, state.milestones.clone())
    };
    let mut ms = serde_json::Map::new();
    for name in ["first_document", "page_loaded", "server_up", "window_created", "window_loaded"] {
        let value = match milestones.iter().find(|(seen, _)| *seen == name) {
            Some((_, elapsed)) => serde_json::Value::from(*elapsed),
            None => serde_json::Value::Null,
        };
        ms.insert((*name).to_string(), value);
    }
    serde_json::json!({
        "milestones_ms": ms,
        "timed_out": timed_out,
        "version": server::VERSION,
    })
}

/// `write_startup_probe()` (`readmd.py:245-283`): print the compact JSON, then
/// atomically replace `--startup-probe-json` through a `.<name>.<pid>.tmp`
/// sibling.  Returns false when the persist failed (Python lets the exception
/// escape and the caller exits 1).
fn probe_write(report: &serde_json::Value, path: Option<&str>) -> bool {
    let encoded = serde_json::to_string(report).unwrap_or_else(|_| "{}".to_string());
    println!("{}", encoded);
    let Some(target) = path else { return true };
    let file = PathBuf::from(target);
    if let Some(parent) = file.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = file.with_file_name(format!(".{}.{}.tmp", name, std::process::id()));
    if std::fs::write(&tmp, format!("{}\n", encoded)).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return false;
    }
    if std::fs::rename(&tmp, &file).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return false;
    }
    true
}

// ---------------------------------------------------------- one-shot modes

/// `install_association()` (`readmd.py:6006-6063`) for `--assoc`
/// (`readmd.py:6459-6462`): HKCU-only registration, no admin, then exit.
/// Non-Windows platforms print the same "set it up yourself" guidance the
/// legacy function returns and exit non-zero, because its result is a string
/// rather than `True`.  Only `APP_DIR`/`DATA_DIR` are needed, so this never
/// touches the store.
///
/// Where the authority spawned `reg.exe` eight times and `ie4uinit.exe -show`
/// once, this goes through `win_registry`: the same keys, value types and
/// payloads as data, `RegCreateKeyExW`/`RegSetValueExW` per key, and one
/// `SHChangeNotify(SHCNE_ASSOCCHANGED)` for the shell refresh.  Zero children.
fn associate_markdown(paths: &AppPaths) -> (bool, String) {
    #[cfg(target_os = "windows")]
    {
        // Frozen build: `pyw = sys.executable`, `cmd = _quote(pyw) + ' "%1"'`
        // (`readmd.py:6026-6028`).
        let exe = match std::env::current_exe() {
            Ok(path) => path,
            Err(err) => return (false, format!("cannot resolve the executable: {}", err)),
        };
        // `icon_file` is copied next to the settings; a missing source or a failed
        // copy takes the whole function down through the outer `except`
        // (`readmd.py:6020-6025`).
        let icon_file = {
            let source = paths.assets_dir.join("markdown-file.ico");
            let dir = paths.data_dir.join("icons");
            let target = dir.join("markdown-file.ico");
            match std::fs::create_dir_all(&dir).and_then(|_| std::fs::copy(&source, &target)) {
                Ok(_) => target.to_string_lossy().into_owned(),
                Err(err) => return (false, format!("markdown-file.ico: {}", err)),
            }
        };
        // The eight `reg add` arguments of `readmd.py:6044-6055` as data: keys,
        // `/ve` (default value), `/t` types and `/d` payloads are all decided by
        // the pure planner, which is what the `win_registry` tests pin.
        let writes =
            win_registry::association_writes(&exe.to_string_lossy(), &icon_file);
        // Python runs every `reg add` through `subprocess.run(capture_output=True)`,
        // which looks at a non-zero exit code and discards it: only a failed
        // *spawn* raises, and only that raise turns into the returned error
        // string.  There is no spawn left to fail, so the per-key `LSTATUS`
        // values reported here are collected and deliberately swallowed — the
        // same swallow, one layer down.  Surfacing them would make the kernel
        // report failure for a state the authority reports as `True`, which is
        // why `_failures` is a binder and not a `?`.
        let _failures = win_registry::apply(&writes);
        // Shell association/icon cache refresh (`readmd.py:6056-6059`).  The
        // `try/except: pass` around the `ie4uinit.exe -show` spawn swallowed
        // every failure it could report; `SHChangeNotify` has no failure
        // channel at all, so the observable contract is unchanged and the
        // child process is gone.
        win_registry::notify_association_changed();
        // `(True, "OK")` is what `install_association()` returns on every path
        // that reaches the end of its `try`, whether or not the individual
        // writes reported an error.
        return (true, "OK".to_string());
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = paths;
        #[cfg(target_os = "macos")]
        return (
            false,
            "macOS 不支持自动注册文件关联。请右键 .md 文件 → 显示简介 →\
             打开方式 → 选择 ReadMD → 全部更改"
                .to_string(),
        );
        #[cfg(not(target_os = "macos"))]
        return (false, "Linux 请使用 xdg-mime 手动设置 .md 文件关联".to_string());
    }
}

/// `--mods` (`readmd.py:6470-6477`): force-load every declared module and print
/// `name: state`; exit 0 only when all of them report usable.
fn run_mods(app: Arc<App>) -> i32 {
    let port = match server::spawn(app.clone(), "127.0.0.1", 0) {
        Ok(port) => port,
        Err(err) => {
            eprintln!("readmd: cannot start the module probe ({})", err);
            return 1;
        }
    };
    let mut ok = true;
    for name in readmd_kernel::modules::MODULES {
        let body = loopback_body(
            port,
            "GET",
            &format!("/api/modules/load?name={}", name),
            None,
            Duration::from_secs(20),
        );
        let status = json_field(&body, "status").unwrap_or_else(|| "error".to_string());
        let note = json_field(&body, "detail").unwrap_or_default();
        println!("{}: {}{}", name, status, if note.is_empty() { String::new() } else { format!(" - {}", note) });
        ok = ok && status == "ready";
    }
    clear_instance(&app.paths.data_dir);
    if ok {
        0
    } else {
        1
    }
}

/// Assert a condition the same way `run_selftest()` does: print and keep the
/// failure (`readmd.py:6088-6288`).
fn selftest_check(ok: &mut bool, area: &str, result: Result<(), String>, passed: &str) {
    match result {
        Ok(()) => println!("{}", passed),
        Err(err) => {
            println!("{} selftest failed: {}", area, err);
            *ok = false;
        }
    }
}

fn asset_min(assets: &Path, rel: &str, min_bytes: u64) -> Result<(), String> {
    let file = assets.join(rel);
    let meta = std::fs::metadata(&file).map_err(|_| format!("{} missing", file.display()))?;
    if meta.len() < min_bytes {
        return Err(format!("{} is {} bytes, expected > {}", file.display(), meta.len(), min_bytes));
    }
    Ok(())
}

/// Bundled reader resources and the association icon — the same files
/// `readmd.py:6090-6110` asserts on before anything else runs.
fn selftest_assets(app: &App) -> Result<(), String> {
    let assets = &app.paths.assets_dir;
    asset_min(assets, "vendor/readability.bundle.js", 10_000)?;
    asset_min(assets, "vendor/readability.LICENSE.md", 400)?;
    asset_min(assets, "vendor/defuddle.bundle.js", 100_000)?;
    asset_min(assets, "vendor/defuddle.LICENSE.txt", 500)?;
    let file_icon = assets.join("markdown-file.ico");
    let bytes = std::fs::read(&file_icon).map_err(|_| "markdown-file.ico missing".to_string())?;
    if bytes.len() <= 1000 {
        return Err("markdown-file.ico is too small".to_string());
    }
    if bytes[..4] != [0, 0, 1, 0] {
        return Err("markdown-file.ico has no ICONDIR header".to_string());
    }
    let app_icon = assets.join("readmd.ico");
    if app_icon.is_file() {
        if let Ok(app_bytes) = std::fs::read(&app_icon) {
            if app_bytes == bytes {
                return Err("markdown-file.ico and readmd.ico are identical".to_string());
            }
        }
    }
    Ok(())
}

/// Boot a private kernel on an ephemeral port, read the page and the module
/// contract (`readmd.py:6148-6166`).
fn selftest_http(app: &Arc<App>) -> Result<(), String> {
    let port = server::spawn(app.clone(), "127.0.0.1", 0).map_err(|e| e.to_string())?;
    let page = loopback_exchange(port, "GET", "/", None, &[], Duration::from_secs(5))
        .ok_or_else(|| "no answer from the kernel".to_string())?;
    if page.0 != 200 || !page.1.contains("ReadMD") {
        return Err(format!("GET / -> {} without a ReadMD document", page.0));
    }
    let modules = loopback_body(port, "GET", "/api/modules", None, Duration::from_secs(10))
        .ok_or_else(|| "GET /api/modules timed out".to_string())?;
    let value: serde_json::Value =
        serde_json::from_str(&modules).map_err(|e| format!("/api/modules: {e}"))?;
    if value.get("modules").and_then(|m| m.get("ai")).is_none() {
        return Err("/api/modules has no ai entry".to_string());
    }
    println!("http server OK (port {})", port);
    Ok(())
}

/// The single-instance control round trip of `readmd.py:6168-6198`: publish an
/// `instance.json` token, then require `/api/ping?t=` and
/// `/api/control/open` to agree on it.  The previous file contents are restored
/// either way, exactly like `save_json(INSTANCE_FILE, old_inst)`.
fn selftest_control(app: &Arc<App>) -> Result<(), String> {
    let port = server::spawn(app.clone(), "127.0.0.1", 0).map_err(|e| e.to_string())?;
    let file = instance_path(&app.paths.data_dir);
    let previous = std::fs::read(&file).ok();
    let token = format!("selftest-{:08x}", rand_u32());
    let value = serde_json::json!({
        "port": port,
        "token": token,
        "pid": std::process::id(),
        "started": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0),
    });
    let _ = std::fs::create_dir_all(&app.paths.data_dir);
    std::fs::write(&file, value.to_string()).map_err(|e| format!("instance.json: {e}"))?;
    let outcome = (|| -> Result<(), String> {
        if !ping_instance(port, &token) {
            return Err("ping with the published token was refused".to_string());
        }
        if ping_instance(port, "bad") {
            return Err("ping accepted a wrong token".to_string());
        }
        let wrong = serde_json::json!({ "token": "bad", "file": "" }).to_string();
        if body_is_ok(&loopback_body(port, "POST", "/api/control/open", Some(&wrong), Duration::from_secs(5))) {
            return Err("control/open accepted a wrong token".to_string());
        }
        let right = serde_json::json!({ "token": token, "file": "" }).to_string();
        if !body_is_ok(&loopback_body(port, "POST", "/api/control/open", Some(&right), Duration::from_secs(5))) {
            return Err("control/open rejected the published token".to_string());
        }
        let first = loopback_body(port, "GET", "/api/control/next", None, Duration::from_secs(5));
        let pending = json_field(&first, "pending").as_deref() == Some("true");
        if !pending || json_field(&first, "file").as_deref() != Some("") {
            return Err("control/next did not return the queued open".to_string());
        }
        let second = loopback_body(port, "GET", "/api/control/next", None, Duration::from_secs(5));
        if json_field(&second, "pending").as_deref() == Some("true") {
            return Err("control/next still reports a pending item".to_string());
        }
        println!("single-instance control selftest OK");
        Ok(())
    })();
    match previous {
        Some(raw) => {
            let _ = std::fs::write(&file, raw);
        }
        None => {
            let _ = std::fs::remove_file(&file);
        }
    }
    outcome
}

fn rand_u32() -> u32 {
    let mut buf = [0u8; 4];
    if getrandom::fill(&mut buf).is_err() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        buf = (nanos as u32).to_le_bytes();
    }
    u32::from_le_bytes(buf)
}

/// `run_selftest()` (`readmd.py:6088`): returns the exit code, `sys.exit()`.
fn run_selftest(app: Arc<App>) -> i32 {
    let mut ok = true;
    selftest_check(
        &mut ok,
        "web extraction resource",
        selftest_assets(&app),
        "web extraction and file association resources OK",
    );
    selftest_check(&mut ok, "http", selftest_http(&app), "");
    selftest_check(&mut ok, "control", selftest_control(&app), "");
    if ok {
        0
    } else {
        1
    }
}

/// `run_webview_selftest()` (`readmd.py:6290-6386`): prove the native webview
/// network guard.  The legacy harness loads a real page and asserts it cannot
/// reach a private-network probe; the kernel-side equivalent of that property is
/// the loopback guard in `server::authorize` (`readmd.py:1101-1114`,
/// `readmd.py:1146-1163`), which is exercised here over a real socket: a
/// same-origin loopback request must pass while a forged `Host` or foreign
/// `Origin` must be refused before any handler runs.
fn run_webview_selftest(app: Arc<App>) -> i32 {
    let port = match server::spawn(app.clone(), "127.0.0.1", 0) {
        Ok(port) => port,
        Err(err) => {
            println!("webview network guard FAILED: cannot bind ({})", err);
            return 1;
        }
    };
    let same_origin = loopback_exchange(
        port,
        "GET",
        "/api/recent/status",
        None,
        &[("Host", &format!("127.0.0.1:{}", port)[..])],
        Duration::from_secs(5),
    );
    let forged_host = loopback_exchange(
        port,
        "GET",
        "/api/recent/status",
        None,
        &[("Host", "readmd.invalid")],
        Duration::from_secs(5),
    );
    let foreign_origin = loopback_exchange(
        port,
        "POST",
        "/api/settings",
        Some("{}"),
        &[("Origin", "http://evil.example")],
        Duration::from_secs(5),
    );
    let detail = match (same_origin, forged_host, foreign_origin) {
        (Some((base, _)), Some((403, _)), Some((403, _))) if base != 403 => None,
        (Some((base, _)), _, _) if base == 403 => {
            Some("same-origin loopback request was refused".to_string())
        }
        (_, Some((code, _)), _) if code != 403 => {
            Some(format!("forged Host answered {} instead of 403", code))
        }
        (_, _, Some((code, _))) if code != 403 => {
            Some(format!("cross-origin POST answered {} instead of 403", code))
        }
        _ => Some("the guard probes did not answer".to_string()),
    };
    clear_instance(&app.paths.data_dir);
    match detail {
        None => {
            println!("webview network guard PASSED");
            0
        }
        Some(error) => {
            println!("webview network guard FAILED: {}", error);
            1
        }
    }
}

/// `--diagnose` / `--check-*` (`readmd.py:6421-6436`, reports from
/// `src/readmd_modules/{system,windows,linux,macos}_native.py`).  The section
/// order and the `[N]` labels mirror the legacy report; the values come from the
/// Rust process itself.
fn diagnose_report(target: Check) -> String {
    let flavor = match target {
        Check::Linux => "linux",
        Check::Windows => "windows",
        Check::MacOS => "macos",
        Check::Diagnose => std::env::consts::OS,
    };
    let mut lines: Vec<String> = vec!["=".repeat(64)];
    lines.push(format!(
        " ReadMD {} native environment and graphics engine diagnosis",
        flavor
    ));
    lines.push("=".repeat(64));
    lines.push(format!(
        "[N] ReadMD version: {} (engine: rust, kernel {})",
        server::VERSION,
        server::SERVER_TAG
    ));
    lines.push(format!(
        "[N] operating system: {} ({} {}, family {})",
        std::env::consts::OS,
        flavor,
        std::env::consts::ARCH,
        std::env::consts::FAMILY
    ));
    lines.push(format!("[N] processor architecture: {}", std::env::consts::ARCH));
    if let Ok(ptr) = std::env::var("PROCESSOR_ARCHITECTURE") {
        lines.push(format!("[N] Windows on ARM: {}", ptr.to_ascii_uppercase() == "ARM64"));
    }
    lines.push("-".repeat(64));
    lines.push("[N] rendering engine probe:".to_string());
    #[cfg(target_os = "windows")]
    {
        let (installed, version, path) = webview2_runtime();
        if installed {
            lines.push(format!(
                "  - Microsoft Edge WebView2 runtime: ready (version {}, path {})",
                version, path
            ));
        } else {
            lines.push(
                "  - Microsoft Edge WebView2 runtime: not installed (falls back to Browser App mode)"
                    .to_string(),
            );
        }
        lines.push(format!(
            "  - standalone Browser App mode (msedge/chrome): {}",
            match browser_app_candidate() {
                Some(found) => format!("ready ({})", found),
                None => "no compatible browser found".to_string(),
            }
        ));
        lines.push(format!(
            "[N] overall readiness: {}",
            if installed {
                "[OK] native webview available"
            } else if browser_app_candidate().is_some() {
                "[WARNING] install the Edge WebView2 runtime for the best experience"
            } else {
                "[WARNING] no native rendering engine found"
            }
        ));
    }
    #[cfg(not(target_os = "windows"))]
    {
        lines.push(format!(
            "  - native webview backend: {} ({})",
            if cfg!(target_os = "macos") { "Cocoa WKWebView" } else { "WebKitGTK / QtWebEngine" },
            "resolved by the system webview at run time"
        ));
        lines.push(format!(
            "  - display available: {}",
            std::env::var("DISPLAY")
                .or_else(|_| std::env::var("WAYLAND_DISPLAY"))
                .map(|v| format!("yes ({})", v))
                .unwrap_or_else(|_| "no".to_string())
        ));
        lines.push("[N] overall readiness: [OK] native webview path selected".to_string());
    }
    lines.push("=".repeat(64));
    lines.join("\n")
}

/// The Edge WebView2 (EC) client GUID `windows_native.py:100` probes.
#[cfg(target_os = "windows")]
const EC_GUID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

/// The two EC client registrations [`webview2_runtime`] consults, in the order
/// the retired `reg query` loop tried them, as `(root, sub-key)` pairs now that
/// there is no `HKLM\…` string for a child process to parse.
///
/// No `KEY_WOW64_32KEY`/`KEY_WOW64_64KEY` bit is requested anywhere, and that is
/// the parity choice: the retired `reg query` child ran in this process'
/// bitness (64-bit on every supported host), and a 64-bit reader with no view
/// flag gets the 64-bit view — which is precisely where the literal
/// `SOFTWARE\WOW6432Node\…` path points.  Asking for the 32-bit view instead
/// would have resolved the same *name* to `SOFTWARE\Microsoft\EdgeUpdate\…` and
/// changed what gets read.
#[cfg(target_os = "windows")]
const EC_CLIENT_KEYS: [(usize, &str); 2] = [
    (win_registry::HKLM, r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\"),
    (win_registry::HKCU, r"Software\Microsoft\EdgeUpdate\Clients\"),
];

/// The version value name, and the folder value name, both read as `REG_SZ`.
#[cfg(target_os = "windows")]
const EC_VERSION_VALUE: &str = "pv";
#[cfg(target_os = "windows")]
const EC_FOLDER_VALUE: &str = "InstallationFolder";

/// `windows_native.diagnose_system()['webview2']`: the installed version and
/// path are read from the same EC client registration the legacy module probes,
/// through `RegOpenKeyExW`/`RegQueryValueExW` instead of a `reg query` child.
#[cfg(target_os = "windows")]
fn webview2_runtime() -> (bool, String, String) {
    for (root, prefix) in EC_CLIENT_KEYS {
        let sub_key = format!("{}{}", prefix, EC_GUID);
        // `query_reg_sz` is the type filter that made the retired parser skip
        // every line that did not print `REG_SZ`, and the trim/emptiness rules
        // below are its `value.trim()` and `.filter(|v| !v.is_empty())`.  Both
        // names are looked up case-insensitively, which is what the parser's
        // lower-cased name map intended; measured on this host the two keys
        // carry `name`, `pv`, `location` and `SilentUninstall`, so the only
        // value that could resolve differently from the retired text path is an
        // `InstallationFolder` that does not exist.
        let get = |name: &str| {
            win_registry::query_reg_sz(root, &sub_key, name)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_default()
        };
        let version = get(EC_VERSION_VALUE);
        let path = get(EC_FOLDER_VALUE);
        if !version.is_empty() || !path.is_empty() {
            return (true, version, path);
        }
    }
    (false, String::new(), String::new())
}

/// Lane `port-registry-ffi-s14` gates for the two call sites that used to spawn
/// `reg.exe` / `ie4uinit.exe`.  These read this file's own text, so they assert
/// the *shape* of the implementation — no child process — without touching the
/// registry, a process table or the clock.
#[cfg(all(test, target_os = "windows"))]
mod registry_ffi_tests {
    use super::*;

    fn source() -> String {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"))
            .expect("the bin can read its own source")
    }

    /// Text from `marker` up to the next column-zero `}` — i.e. one item body.
    fn item_body(marker: &str) -> String {
        let text = source();
        let start = text.find(marker).unwrap_or_else(|| panic!("no {}", marker));
        let rest = &text[start..];
        let end = rest
            .find("\n}\n")
            .unwrap_or_else(|| panic!("unterminated body for {}", marker));
        rest[..end + 1].to_string()
    }

    #[test]
    fn association_and_probe_call_sites_spawn_nothing() {
        // The bodies may still *talk about* `reg.exe` and `ie4uinit.exe` in
        // their parity comments; what they may not do is reach for a process.
        for marker in ["fn associate_markdown", "fn webview2_runtime"] {
            let body = item_body(marker);
            for forbidden in [
                "silent_command",
                "Command::new",
                "std::process",
                "spawn(",
                ".output()",
                ".status()",
            ] {
                assert!(
                    !body.contains(forbidden),
                    "{} still reaches for {}",
                    marker,
                    forbidden
                );
            }
            // …and it does reach for the FFI module instead.
            assert!(body.contains("win_registry::"), "{} lost win_registry", marker);
        }
    }

    #[test]
    fn webview2_probe_reads_the_same_keys_the_child_process_was_given() {
        // `windows_native.py:100`: the client GUID the legacy probe is keyed on.
        assert_eq!(EC_GUID, "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}");
        // The value names are the ones the retired parser read.  Note the folder
        // name is *not* the authority's `location` — see the lane report.
        assert_eq!(EC_VERSION_VALUE, "pv");
        assert_eq!(EC_FOLDER_VALUE, "InstallationFolder");
        // Rebuilt from (root, prefix) pairs, the paths must be exactly the two
        // strings `reg query` used to receive, in that order.
        let full: Vec<String> = EC_CLIENT_KEYS
            .iter()
            .map(|(root, prefix)| {
                let spelled = if *root == win_registry::HKLM {
                    "HKLM"
                } else if *root == win_registry::HKCU {
                    "HKCU"
                } else {
                    "unknown root"
                };
                format!(r"{}\{}{}", spelled, prefix, EC_GUID)
            })
            .collect();
        assert_eq!(
            full,
            [
                r"HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
                r"HKCU\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
            ]
        );
    }
}

/// `windows_native.diagnose_system()['app_browser']`: the same candidate list the
/// browser-app fallback uses.
#[cfg(target_os = "windows")]
fn browser_app_candidate() -> Option<String> {
    [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    ]
    .into_iter()
    .find(|cand| Path::new(cand).is_file())
    .map(|cand| cand.to_string())
}

/// Native-fullscreen request encoding carried in [`SIGNALS`] slot 3.
const FULLSCREEN_IDLE: usize = 0;
const FULLSCREEN_ENTER: usize = 1;
const FULLSCREEN_EXIT: usize = 2;

/// Build the reader webview.
///
/// The first attempt and the isolated-profile retry have to produce exactly the
/// same view, so the builder lives in one factory.  `init_script` is the
/// `window.pywebview` shim: installing it as an *initialization* script is what
/// lets `assets/js/core/state.js:12-24` bind `py`/`hasPy` on the very first
/// pass instead of waiting for `pywebviewready`.
///
/// `Api.report_ready` (`readmd.py:5956`), `Api.show_window` (`readmd.py:5972`),
/// `Api.toggle_native_fullscreen` (`readmd.py:5981`) and `Api.request_quit`
/// (`readmd.py:5995`) are direct native calls against the pywebview `window`
/// object.  The kernel has no Python object to call, so the shim posts a
/// `window.ipc.postMessage` command (wry hands the bare string to the handler as
/// the `Request<String>` body) and the atomics below are the receiving end,
/// consumed by the event loop in `run_window`.
#[allow(clippy::too_many_arguments)]
fn build_webview<'a>(
    web_context: &'a mut wry::WebContext,
    url: &str,
    init_script: &str,
    loaded: Arc<AtomicBool>,
    page_ready: Arc<AtomicBool>,
    quit: Arc<AtomicBool>,
    show: Arc<AtomicBool>,
    fullscreen: Arc<AtomicUsize>,
) -> wry::WebViewBuilder<'a> {
    wry::WebViewBuilder::new_with_web_context(web_context)
        .with_url(url.to_string())
        .with_initialization_script(init_script.to_string())
        // `window.events.loaded += _on_loaded` (`readmd.py:6572-6584`), i.e. the
        // `window_loaded` milestone.
        .with_on_page_load_handler(move |event, _url| {
            if matches!(event, wry::PageLoadEvent::Finished) {
                loaded.store(true, Ordering::SeqCst);
            }
        })
        .with_ipc_handler(move |request: wry::http::Request<String>| {
            match request.body().as_str() {
                "readmd:page-ready" => page_ready.store(true, Ordering::SeqCst),
                "readmd:quit" => quit.store(true, Ordering::SeqCst),
                "readmd:show-window" => show.store(true, Ordering::SeqCst),
                "readmd:fullscreen-on" => {
                    fullscreen.store(FULLSCREEN_ENTER, Ordering::SeqCst)
                }
                "readmd:fullscreen-off" => fullscreen.store(FULLSCREEN_EXIT, Ordering::SeqCst),
                _ => {}
            }
        })
}

/// Boot the native webview and serve until it closes.
///
/// `tao`'s `EventLoop::run` is declared `-> !` and calls `std::process::exit`
/// with the code left in `ControlFlow`, so this function never returns: every
/// terminal path goes through [`shutdown`] first, which is the kernel's single
/// instance.json-tearing-down / probe-reporting tail.
fn run_window(url: String, data_dir: PathBuf, probe: Option<Probe>, probe_json: Option<String>) -> ! {
    use tao::dpi::LogicalSize;
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoop};
    use tao::window::WindowBuilder;
    // Only `WebContext` is named unqualified here; `build_webview` refers to
    // `wry::WebViewBuilder` and `wry::PageLoadEvent` by full path.
    use wry::WebContext;

    let event_loop = EventLoop::new();
    // `webview.create_window('ReadMD', url, width=1160, height=820,
    // min_size=(720,480), ..., background_color='#f7f7f5')`
    // (`readmd.py:6546-6549`).
    let mut win_builder = WindowBuilder::new()
        .with_title("ReadMD")
        .with_visible(false)
        .with_background_color((0xf7, 0xf7, 0xf5, 255))
        .with_inner_size(LogicalSize::new(1160.0, 820.0))
        .with_min_inner_size(LogicalSize::new(720.0, 480.0));

    let icon_bytes = include_bytes!("icon_32.bin");
    if let Ok(icon) = tao::window::Icon::from_rgba(icon_bytes.to_vec(), 32, 32) {
        win_builder = win_builder.with_window_icon(Some(icon));
    }

    let window = win_builder.build(&event_loop);
    let window = match window {
        Ok(w) => w,
        Err(err) => {
            eprintln!("readmd: window failed ({}), launching browser app", err);
            launch_browser_app(&url);
            // `readmd.py:6550-6554`: a window that cannot be created is a failed
            // boot — the probe report is still written and the exit code is 1.
            static FAILED: AtomicBool = AtomicBool::new(false);
            exit(shutdown(&FAILED, &data_dir, &probe, &probe_json, false, true));
        }
    };
    // `milestone('boot', 'window_created')` (`readmd.py:6560`).
    probe_mark(&probe, "window_created");

    let bridge_init_script = r#"
        (function () {
            'use strict';
            // Token supply (R7 §B1).  `Api.save_file` is a native call in Python
            // (`readmd.py:4826`), so the app-token gate never applied to it; the
            // kernel stands in with HTTP, and `do_POST` gates `/api/save` on
            // `X-ReadMD-App-Token` compared against the per-process secret minted
            // at `readmd.py:966` (`readmd.py:1072-1076`).  `serve_index` injects
            // that secret into `<meta name="readmd-app-token">`, which is exactly
            // the source `assets/js/core/state.js:27` reads for `apiFetch`, so the
            // bridge reads the same place at call time.  `X-ReadMD-Token` mirrors
            // `readmd.py:1109-1113` and is what makes the kernel-only
            // `--require-token` switch usable in the window at all.
            function bridgeHeaders(json) {
                var h = {};
                if (json) { h['Content-Type'] = 'application/json'; }
                try {
                    var meta = document.querySelector('meta[name="readmd-app-token"]');
                    if (meta && meta.content) { h['X-ReadMD-App-Token'] = meta.content; }
                } catch (e) {}
                try {
                    if (window.LAN_TOKEN) { h['X-ReadMD-Token'] = window.LAN_TOKEN; }
                } catch (e) {}
                return h;
            }
            // JS twin of `safe_external_url()`
            // (`src/readmd_core/safe_open.py:55-71`) + `validate_url()`'s scheme
            // rule (`src/readmd_modules/validators.py:110-115`): reject
            // empty/oversized/control-character values, accept `mailto:` only
            // with an `@` in the address and no whitespace anywhere, and require
            // http/https plus a hostname otherwise.  Returns the cleaned URL or
            // `null`, where Python raises and `Api.open_external` turns that into
            // `False`.
            function bridgeSafeExternalUrl(url) {
                if (!url || typeof url !== 'string') { return null; }
                var value = url.trim();
                if (!value || value.length > 2048) { return null; }
                for (var i = 0; i < value.length; i++) {
                    if (value.charCodeAt(i) < 32) { return null; }
                }
                var scheme = /^([a-zA-Z][a-zA-Z0-9+.-]*):/.exec(value);
                var head = scheme ? scheme[1].toLowerCase() : '';
                if (head === 'mailto') {
                    var rest = value.slice(7);
                    if (rest.indexOf('@') < 0) { return null; }
                    if (/\s/.test(value)) { return null; }
                    return value;
                }
                if (head !== 'http' && head !== 'https') { return null; }
                var authority = /^https?:\/\/([^\/?#]*)/.exec(value);
                if (!authority) { return null; }
                var host = authority[1].replace(/:[0-9]*$/, '').replace(/^[^@]*@/, '');
                if (!host) { return null; }
                return value;
            }
            // wry's IPC channel is one-way (`window.ipc.postMessage` returns
            // nothing), so a *toggle* has to be resolved to an explicit
            // on/off message on this side.  The mirror is what Python does not
            // need: `Api.toggle_native_fullscreen()` (`readmd.py:5981`) asks the
            // host window object for the flip.  `assets/js/reader/render.js:2549`
            // keeps its own `window.__readmdNativeFullscreen` copy of the same
            // bit, so both sides only ever drift the way they already drift in
            // the Python app (an OS-level exit from borderless fullscreen).
            var nativeFullscreen = false;
            window.pywebview = {
            api: {
                choose_folder: async function() {
                    try {
                        const res = await fetch('/api/dialog/choose-folder', { method: 'POST', headers: bridgeHeaders(false) });
                        const data = await res.json();
                        return data.path;
                    } catch(e) { return null; }
                },
                choose_file: async function() {
                    try {
                        const res = await fetch('/api/dialog/choose-file', { method: 'POST', headers: bridgeHeaders(false) });
                        const data = await res.json();
                        return data.path;
                    } catch(e) { return null; }
                },
                choose_any_file: async function() {
                    try {
                        const res = await fetch('/api/dialog/choose-any-file', { method: 'POST', headers: bridgeHeaders(false) });
                        const data = await res.json();
                        return data.path;
                    } catch(e) { return null; }
                },
                choose_many_files: async function() {
                    try {
                        const res = await fetch('/api/dialog/choose-many-files', { method: 'POST', headers: bridgeHeaders(false) });
                        const data = await res.json();
                        return data.paths || [];
                    } catch(e) { return []; }
                },
                choose_skill_source: async function(type) {
                    try {
                        const ep = (type === 'zip' || type === 'archive') ? '/api/dialog/choose-file' : '/api/dialog/choose-folder';
                        const res = await fetch(ep, { method: 'POST', headers: bridgeHeaders(false) });
                        const data = await res.json();
                        return data.path;
                    } catch(e) { return null; }
                },
                choose_pet_plugin: async function() {
                    try {
                        const res = await fetch('/api/dialog/choose-file', { method: 'POST', headers: bridgeHeaders(false) });
                        const data = await res.json();
                        return data.path;
                    } catch(e) { return null; }
                },
                save_as: async function(content, suggested, assets) {
                    try {
                        let bodyData;
                        if (assets !== undefined || (typeof content === 'string' && content.length > 50 && typeof suggested === 'string')) {
                            bodyData = { content: content, suggested: suggested || 'document.md', assets: assets || [] };
                        } else if (typeof content === 'string' && !suggested) {
                            bodyData = { content: '', suggested: content, assets: [] };
                        } else {
                            bodyData = { content: content || '', suggested: suggested || 'document.md', assets: assets || [] };
                        }
                        const res = await fetch('/api/dialog/save-as', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify(bodyData)
                        });
                        const data = await res.json();
                        return data.path || null;
                    } catch(e) { return null; }
                },
                // `Api.save_file(path, content, encoding, expected_mtime=None)`
                // (`readmd.py:4826`) forwards all four into `save_text_atomic`;
                // the caller passes exactly that arity
                // (`assets/js/editor/preview.js:453`
                // `py.save_file(state.file, content, state.encoding || 'utf-8', state.mtime || null)`).
                // R7 §B1 is fixed by `bridgeHeaders` (the `X-ReadMD-App-Token` the
                // `/api/save` gate at `readmd.py:1072-1076` demands); R7 §M3 is
                // fixed here by stopping the argument drop.
                save_file: async function(path, content, encoding, expected_mtime) {
                    try {
                        const res = await fetch('/api/save', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({
                                path: path,
                                content: content,
                                encoding: encoding || 'utf-8',
                                expected_mtime: (expected_mtime === undefined ? null : expected_mtime)
                            })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                save_fixed: async function(path, content) {
                    try {
                        const res = await fetch('/api/file/save-fixed', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ path: path, content: content })
                        });
                        const data = await res.json();
                        return data.path || null;
                    } catch(e) { return null; }
                },
                install_association: async function() {
                    try {
                        const res = await fetch('/api/system/assoc', { method: 'POST', headers: bridgeHeaders(false) });
                        const data = await res.json();
                        return data.ok === true ? true : (data.error || false);
                    } catch(e) { return false; }
                },
                authorize_clipboard_read: async function() {
                    return { ok: true, token: 'direct-auth' };
                },
                read_clipboard: async function(_token) {
                    try {
                        const res = await fetch('/api/clipboard/read', { method: 'POST', headers: bridgeHeaders(false) });
                        return await res.json();
                    } catch(e) {
                        return { text: '', html: '', files: [], image: '', image_path: '', source_type: 'empty', error: e.message };
                    }
                },
                get_autostart: async function() {
                    try {
                        const res = await fetch('/api/autostart/get', { headers: bridgeHeaders(false) });
                        const data = await res.json();
                        return !!data.enabled;
                    } catch(e) { return false; }
                },
                set_autostart: async function(enabled) {
                    try {
                        const res = await fetch('/api/autostart/set', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ enabled: !!enabled })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                get_recent: async function() {
                    try {
                        const res = await fetch('/api/recent/status', { headers: bridgeHeaders(false) });
                        const data = await res.json();
                        return (data.recent || []).map(r => r.path || r);
                    } catch(e) { return []; }
                },
                add_recent: async function(path) {
                    try {
                        await fetch('/api/recent/add', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ path: path })
                        });
                        return true;
                    } catch(e) { return false; }
                },
                remove_recent: async function(path) {
                    try {
                        await fetch('/api/recent/remove', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ path: path })
                        });
                        return true;
                    } catch(e) { return false; }
                },
                clear_recent: async function() {
                    try {
                        await fetch('/api/recent/clear', { method: 'POST', headers: bridgeHeaders(false) });
                        return true;
            } catch(e) { return false; }
                },
                check_recent_status: async function(paths) {
                    try {
                        const res = await fetch('/api/recent/status', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ paths: paths || [] })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, recent: [] }; }
                },
                // `Api.get_settings()` returns the settings JSON **bare**
                // (`readmd.py:5030` `return load_json(SETTINGS_FILE, {})`), and
                // `assets/js/core/settings.js:11` feeds it straight into
                // `Object.assign(state, s)`.  The agreed wave-B contract is that
                // `GET /api/settings` answers the bare map (`save_settings` keeps
                // merging, `readmd.py:5792`); this returns exactly what the route
                // gives, and unwraps only a genuine `{ok,settings}` envelope so a
                // preference can never be silently dropped again.  No key is
                // invented or renamed on either side.
                get_settings: async function() {
                    try {
                        const res = await fetch('/api/settings', { headers: bridgeHeaders(false) });
                        const data = await res.json();
                        if (data && typeof data === 'object' && !Array.isArray(data) &&
                                typeof data.ok === 'boolean' &&
                                data.settings && typeof data.settings === 'object') {
                            return data.settings;
                        }
                        return data || {};
                    } catch(e) { return {}; }
                },
                save_settings: async function(settings) {
                    try {
                        const res = await fetch('/api/settings', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify(settings || {})
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                get_custom_styles: async function() {
                    try {
                        const res = await fetch('/api/style/get', { headers: bridgeHeaders(false) });
                        return await res.json();
                    } catch(e) { return { ok: false, data: {} }; }
                },
                save_custom_styles: async function(css, head) {
                    try {
                        const res = await fetch('/api/style/save', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ css: css || '', head: head || '' })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                export_presentation: async function(content, theme, transition, save) {
                    try {
                        const res = await fetch('/api/export/presentation', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ content: content, theme: theme || 'black', transition: transition || 'slide', save: !!save })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                export_epub: async function(content, out_path, meta, options) {
                    try {
                        const res = await fetch('/api/export/epub', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ confirm: true, content: content, out_path: out_path, meta: meta, options: options })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                export_doc: async function(fmt, payload) {
                    try {
                        const res = await fetch('/api/export', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify(Object.assign({ format: fmt }, payload || {}))
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                // `Api.run_code_chunk(self, lang, code, cwd=None, timeout=10,
                // confirm=False)` (`readmd.py:4362`) is called as
                // `py.run_code_chunk(lang, code, null, 10, true)`
                // (`assets/js/reader/render.js:1729`).  The shim used to bind the
                // first argument to `code`, i.e. it ran the literal string
                // "python" as a program (R7 §B3).  Python also refuses unless
                // `confirm is not True` is false (`readmd.py:4364-4367`), so the
                // caller's flag is forwarded instead of hard-coded.
                run_code_chunk: async function(lang, code, cwd, timeout, confirm) {
                    try {
                        const res = await fetch('/api/code/run', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ confirm: confirm === true, code: code, lang: lang || 'python', cwd: cwd, timeout: timeout || 10 })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                render_diagram: async function(diagramType, code, optionsOrFormat) {
                    try {
                        const opts = (optionsOrFormat && typeof optionsOrFormat === 'object') ? optionsOrFormat : {};
                        const allowRemote = (opts.allow_remote !== false);
                        const format = (typeof optionsOrFormat === 'string') ? optionsOrFormat : (opts.format || 'svg');
                        const res = await fetch('/api/diagram/render', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({
                                engine: diagramType,
                                type: diagramType,
                                code: code,
                                format: format,
                                allow_remote: allowRemote,
                                options: opts
                            })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                get_diagram_capabilities: async function() {
                    try {
                        const res = await fetch('/api/diagram/capabilities', { headers: bridgeHeaders(false) });
                        return await res.json();
                    } catch(e) { return { ok: false, capabilities: {} }; }
                },
                transcribe_file: async function(path, model, language) {
                    try {
                        const res = await fetch('/api/transcribe', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ path: path, model: model || 'base', language: language })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                // `Api.process_imports(self, content, base_dir='',
                // current_file=None)` (`readmd.py:4447`) answers
                // `{'ok':True,'content':<expanded markdown>}` and is called as
                // `py.process_imports(mdText, dir, filePath)`
                // (`assets/js/reader/render.js:850`), consumed through
                // `(res && res.ok) ? res.content : mdText`.  The shim used to post
                // the whole document under `files`, which the handler reads as a
                // *path array*, so the reply carried no `content` key at all and
                // every `@import` document rendered `undefined` (R7 §B4).
                process_imports: async function(content, base_dir, current_file) {
                    try {
                        const res = await fetch('/api/import/process', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ content: content || '', base_dir: base_dir || '', current_file: current_file || null })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                // `Api.extract_zip_batch(self, zip_path)` (`readmd.py:4486`) is a
                // native call and carries no confirmation field, but the kernel
                // route requires `confirm === true` in the JSON body, exactly like
                // the front end's own HTTP twin
                // (`assets/js/core/dragdrop.js:96-100` sends `{path, confirm:true}`),
                // because dropping an archive writes files into `DATA_DIR/temp_zip`.
                // Without it every ZIP drag answered 400 confirmation_required
                // (R7 §B9).
                extract_zip_batch: async function(path) {
                    try {
                        const res = await fetch('/api/batch/extract-zip', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ path: path, confirm: true })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                // `Api.get_links_graph(self, directory='', max_nodes=500)`
                // (`readmd.py:4309`) answers `{'ok': True, 'graph':
                // indexer.get_graph_data(...)}`, and the modal's own guard reads
                // that shape literally: `if (!res?.ok || !res.graph) throw`
                // (`assets/js/features/graph.js:243`).  R7 §B7: the kernel's
                // `h_links_graph` inserts `ok` at the top level of a flat
                // `{nodes,edges,dir}` body, so 知识图谱 could never open.  Wrap a
                // flat body here and pass an already-wrapped one through --
                // correct on either side of the handler fix; Python's failure
                // shape `{'ok': False, 'error': ...}` is forwarded untouched so
                // the modal still reports why.  `max_nodes` is sent because
                // Python's signature carries it; the handler has to grow the cap.
                get_links_graph: async function(dir, max_nodes) {
                    try {
                        const res = await fetch('/api/links/graph?dir=' + encodeURIComponent(dir || '') + '&max_nodes=' + (max_nodes || 500), { headers: bridgeHeaders(false) });
                        const data = await res.json();
                        if (!data || typeof data !== 'object') {
                            return { ok: false, error: 'graph_unavailable' };
                        }
                        if (data.graph || data.ok === false) { return data; }
                        return {
                            ok: true,
                            graph: {
                                nodes: data.nodes || [],
                                edges: data.edges || [],
                                dir: data.dir || dir || ''
                            }
                        };
                    } catch(e) { return { ok: false, graph: { nodes: [], edges: [] }, error: e.message }; }
                },
                get_backlinks: async function(path) {
                    try {
                        const res = await fetch('/api/links/backlinks?path=' + encodeURIComponent(path || ''), { headers: bridgeHeaders(false) });
                        return await res.json();
                    } catch(e) { return { ok: false, backlinks: [], forward_links: [] }; }
                },
                index_directory_links: async function(dir, force) {
                    try {
                        const res = await fetch('/api/links/index', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ dir: dir, force: !!force })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                // `Api.get_bibtex(self, file_path)` (`readmd.py:5023`) returns
                // `bibtex.find_and_load_bib_for_file(file_path)` -- the **bare**
                // map `{citekey: fields}`, where each value is the parsed field
                // dict plus `entry_type`, `cite_key`, `short_cite` and
                // `full_reference` (`src/readmd_modules/bibtex.py:63-71`) -- and
                // `{}` when anything raises.  `assets/js/reader/render.js:320`
                // assigns that return value straight to `currentDocCitations`,
                // then reads `entry.short_cite` (`render.js:1543`), `entry.title`
                // /`author`/`year`/`journal`/`booktitle` (`render.js:1606-1614`),
                // so anything else -- e.g. R7 §B8's `{ok, entries:[], count}`
                // envelope -- resolves zero citations.  Flatten the kernel's
                // `entries[]` (`server.rs parse_bibtex` -> `{type,key,fields}`)
                // into Python's map shape and pass a bare map / `citations`
                // envelope through untouched, so the shim is right whichever side
                // of the handler fix lands first.
                get_bibtex: async function(path) {
                    try {
                        const res = await fetch('/api/bibtex?p=' + encodeURIComponent(path || ''), { headers: bridgeHeaders(false) });
                        const data = await res.json();
                        if (!data || typeof data !== 'object') { return {}; }
                        if (Array.isArray(data.entries)) {
                            const map = {};
                            for (const e of data.entries) {
                                if (!e || typeof e !== 'object') { continue; }
                                const key = e.key || e.cite_key || e.id;
                                if (!key) { continue; }
                                map[key] = Object.assign({}, e.fields || {}, {
                                    entry_type: e.type || e.entry_type || '',
                                    cite_key: key
                                });
                            }
                            return map;
                        }
                        if (data.citations && typeof data.citations === 'object') { return data.citations; }
                        if ('ok' in data) { return {}; }
                        return data;
                    } catch(e) { return {}; }
                },
                // `Api.check_update()` (`readmd.py:4996`) refuses to touch the
                // network while a startup probe owns the run --
                // `if _STARTUP_PROBE.get('enabled'): return {'ok': False,
                // 'error_code': 'probe_mode'}`, mirrored by
                // `Handler._api_update_check` (`readmd.py:1540`) -- so a
                // benchmark never depends on GitHub being reachable.
                // `@@READMD_PROBE_MODE@@` is spliced from `run_window`'s
                // `probe` argument, the kernel's own `_STARTUP_PROBE['enabled']`.
                //
                // The payload contract is `updater.check_update`'s flat dict
                // (`src/readmd_modules/updater.py:446-463`): `ok`, `has_update`,
                // `current_version`, `latest_version`, `release_name`,
                // `published_at`, `release_notes`, `html_url`, `flavor`,
                // `asset{name,size,download_url,expected_sha}`, `sha_url`, read
                // key by key by `assets/js/features/updater.js:20-45`,
                // `updater.js:81-93`, `updater.js:115-145` and
                // `updater.js:195`.  R7 §B6: the kernel's `h_update_check`
                // answers with its own `UpdateStatus` struct
                // (`{available,version,download_url,sha256,downloaded,
                // install_ready}`) -- no `ok`, so 检查更新 always said
                // "检查失败" -- until that handler is fixed; mapping it here and
                // passing a Python-shaped body through untouched keeps the shim
                // correct on either side of the server fix.  `flavor` must stay
                // whatever the server detected because `_validate_ready_update`
                // (`updater.py:782`) rejects a mismatched one.
                check_update: async function() {
                    if (@@READMD_PROBE_MODE@@) { return { ok: false, error_code: 'probe_mode' }; }
                    try {
                        const res = await fetch('/api/update/check', { headers: bridgeHeaders(false) });
                        const data = await res.json();
                        if (!data || typeof data !== 'object') {
                            return { ok: false, error_code: 'update_response_invalid' };
                        }
                        if (typeof data.ok === 'boolean') { return data; }
                        const latest = data.version ? String(data.version) : '';
                        return {
                            ok: true,
                            has_update: !!data.available,
                            current_version: '@@READMD_VERSION@@',
                            latest_version: latest,
                            release_name: latest ? ('ReadMD ' + latest) : '',
                            published_at: data.published_at || '',
                            release_notes: data.release_notes || '',
                            html_url: data.html_url || '',
                            flavor: data.flavor || null,
                            asset: data.download_url ? {
                                name: String(data.download_url).split('/').pop(),
                                size: data.size || 0,
                                download_url: data.download_url,
                                expected_sha: data.sha256 || null
                            } : null,
                            sha_url: data.sha_url || null
                        };
                    } catch(e) { return { ok: false, error_code: 'update_network_error', error: e.message }; }
                },
                start_download_update: async function(url, filename, sha, mirror) {
                    try {
                        const res = await fetch('/api/update/download', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ download_url: url, target_filename: filename, expected_sha: sha, use_mirror: !!mirror })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                get_download_status: async function() {
                    try {
                        const res = await fetch('/api/update/status', { headers: bridgeHeaders(false) });
                        return await res.json();
                    } catch(e) { return { status: 'idle' }; }
                },
                cancel_download: async function() {
                    try {
                        const res = await fetch('/api/update/cancel', { headers: bridgeHeaders(false) });
                        return await res.json();
                    } catch(e) { return { ok: false }; }
                },
                apply_update: async function(filePath, flavor) {
                    try {
                        const res = await fetch('/api/update/apply', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ file_path: filePath, flavor: flavor })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                get_system_language: async function() {
                    try {
                        const res = await fetch('/api/system/language', { headers: bridgeHeaders(false) });
                        const data = await res.json();
                        return data.language || 'zh-CN';
                    } catch(e) { return 'zh-CN'; }
                },
                get_app_info: async function() {
                    return { version: '@@READMD_VERSION@@', engine: 'rust-wry', platform: '@@READMD_PLATFORM@@' };
                },
                check_upgrade: async function() {
                    return { ok: false };
                },
                start_modules: async function() {
                    return { ok: true };
                },
                get_modules_status: async function() {
                    try {
                        const res = await fetch('/api/modules', { headers: bridgeHeaders(false) });
                        return await res.json();
                    } catch(e) { return { modules: {} }; }
                },
                get_pet_runtime_status: async function() {
                    try {
                        const res = await fetch('/api/pets/status', { headers: bridgeHeaders(false) });
                        return await res.json();
                    } catch(e) { return { ok: false }; }
                },
                configure_pet: async function(config) {
                    try {
                        const res = await fetch('/api/pets/configure', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify(config || {})
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                interact_pet: async function(action, character) {
                    try {
                        const res = await fetch('/api/pets/interact', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ action: action, character: character })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false }; }
                },
                get_pet_update_status: async function() {
                    try {
                        const res = await fetch('/api/pets/update_status', { headers: bridgeHeaders(false) });
                        return await res.json();
                    } catch(e) { return { ok: false }; }
                },
                check_pet_update: async function(allowNetwork) {
                    try {
                        const res = await fetch('/api/pets/check_update', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ allow_network: allowNetwork !== false })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false }; }
                },
                apply_pet_update: async function() {
                    try {
                        const res = await fetch('/api/pets/apply_update', { method: 'POST', headers: bridgeHeaders(false) });
                        return await res.json();
                    } catch(e) { return { ok: false }; }
                },
                install_companion_pet: async function() {
                    try {
                        const res = await fetch('/api/pets/install', { method: 'POST', headers: bridgeHeaders(false) });
                        return await res.json();
                    } catch(e) { return { ok: false }; }
                },
                uninstall_companion_pet: async function() {
                    try {
                        const res = await fetch('/api/pets/uninstall', { method: 'POST', headers: bridgeHeaders(false) });
                        return await res.json();
                    } catch(e) { return { ok: false }; }
                },
                list_local_pets: async function() {
                    try {
                        const res = await fetch('/api/pets', { headers: bridgeHeaders(false) });
                        return await res.json();
                    } catch(e) { return { ok: false, pets: [] }; }
                },
                import_local_pet: async function(slug, imageBase64, name, desc, replace) {
                    try {
                        const res = await fetch('/api/pets/import', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ confirm: true, slug: slug, image_base64: imageBase64, display_name: name, description: desc, replace: !!replace })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                remove_local_pet: async function(slug) {
                    try {
                        const res = await fetch('/api/pets/remove', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ confirm: true, slug: slug })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false }; }
                },
                set_active_pet: async function(slug) {
                    try {
                        const res = await fetch('/api/pets/active', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ confirm: true, slug: slug })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false }; }
                },
                install_pet_plugin: async function() {
                    try {
                        const res = await fetch('/api/pets/install', { method: 'POST', headers: bridgeHeaders(false) });
                        return await res.json();
                    } catch(e) { return { ok: false }; }
                },
                install_default_pet_plugin: async function() {
                    try {
                        const res = await fetch('/api/pets/runtime/install', { method: 'POST', headers: bridgeHeaders(false) });
                        return await res.json();
                    } catch(e) { return { ok: false }; }
                },
                configure_pet: async function(config) {
                    try {
                        const res = await fetch('/api/pets/configure', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify(config || {})
                        });
                        return await res.json();
                    } catch(e) { return { ok: false }; }
                },
                enqueue_pet_files: async function(files) {
                    try {
                        const res = await fetch('/api/control/pet-batch', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ files: files || [] })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false }; }
                },
                get_pet_batch: async function() {
                    try {
                        const res = await fetch('/api/control/pet-batch', { headers: bridgeHeaders(false) });
                        return await res.json();
                    } catch(e) { return { pending: false, paths: [] }; }
                },
                // R7 §M2.  Python's `Api.render_web_page(url, task_id,
                // timeout_ms, interactive, private_grant, source_html)`
                // (`readmd.py:4551`) really opens a *second* native reader window
                // (`readmd.py:4587`) behind a network guard
                // (`readmd.py:3874`) and hands back
                // `{ok, html, final_url, defuddle, readability}`;
                // `Api.authorize_private_web(url, task_id)` (`readmd.py:3828`)
                // opens a login window and returns `{ok, grant}`.  The kernel has
                // no such renderer -- `POST /api/web/extract` is a plain HTTP
                // fetch, and the old shim's `{ok:true}` from it made
                // `assets/js/features/web.js:107` proceed with
                // `html: rendered.html || ''`, i.e. import an *empty* document
                // while the diagnostics labelled `system-webview` as the engine
                // that produced it.  That is worse than having no renderer, so
                // `render_web_page`/`cancel_web_render` are deliberately not
                // advertised: `web.js:93-98` is the front end's own
                // "no dynamic renderer" guard and raises `render_unavailable`
                // with the honest 该页面需要 JavaScript wording, exactly what the
                // browser-mode client shows.  Cancellation still reaches the
                // kernel, because `cancelWebTask()` (`web.js:141-147`) posts
                // `/api/web/cancel {task_id}` itself.  The grant shims used to
                // return the bare boolean `true`, which looks successful to a
                // caller but carries no `ok`/`grant` member -- `web.js:172-175`
                // reads `authorization.ok` then `authorization.grant`, so
                // `options.privateGrant` stayed empty and 需要登录/内网页面 silently
                // fell back to the unauthenticated fetch.  They now answer
                // Python's object shape with an explicit `ok:false`, and
                // `web.js:173` leaves the grant empty for the honest reason.
                authorize_private_web: async function() {
                    return { ok: false, code: 'renderer_unavailable' };
                },
                revoke_private_web: async function() {
                    return { ok: false, code: 'renderer_unavailable' };
                },
                // `Api.rename_file(self, path, new_stem)` (`readmd.py:4732`)
                // receives a *stem*: `new_path = os.path.join(os.path.dirname(
                // old_path), stem + os.path.splitext(old_path)[1])`, i.e. the
                // directory and the extension of the original file are kept and
                // only the name body changes.  Both JS call sites depend on that
                // spelling (`assets/js/core/tabs.js:379` sends the tab title,
                // `assets/js/reader/render.js:88` sends `input.value.trim()`),
                // while the old `{from,to}` pair read as "rename to this full
                // destination", which is how R7 §B5 lost the folder and the
                // extension.  The kernel handler's own vocabulary is
                // `path` + `new_stem` (`server.rs h_rename`), and stem
                // validation/`target_exists`/reference sync stay server-side
                // exactly as `_validate_rename_stem` keeps them in Python.
                rename_file: async function(path, new_stem) {
                    try {
                        const res = await fetch('/api/rename', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ path: path, new_stem: new_stem })
                        });
                        return await res.json();
                    } catch(e) { return { ok: false, error: e.message }; }
                },
                get_export_presets: async function() { return {}; },
                save_export_presets: async function() { return true; },
                open_path: function(path) {
                    fetch('/api/system/open-path', {
                        method: 'POST',
                        headers: bridgeHeaders(true),
                        body: JSON.stringify({ path: path })
                    }).catch(() => {});
                },
                // `Api.open_external(self, url)` (`readmd.py:4966`) never hands
                // the OS launcher an unchecked string: it first runs
                // `safe_external_url(url)` (`src/readmd_core/safe_open.py:55-71`)
                // and returns `False` after `logging.warning('Blocked unsafe
                // external URL: %s', exc)` when that raises.  The rules there are
                // "non-empty, <=2048 chars, no control character, `mailto:` needs
                // an `@` and no whitespace, everything else goes through
                // `validate_url` which allows only http/https with a hostname"
                // (`src/readmd_modules/validators.py:110`).  R7 §M8: the shim used
                // to post whatever the renderer passed straight into
                // `/api/system/open-path`, which is `explorer`/`cmd /c start` on
                // the raw value -- so `javascript:`, `data:`, `.exe` or
                // `\\nas\share\x` hrefs launched on the desktop where the Python
                // app refuses.  The caller at
                // `assets/js/reader/render.js:2904` already filters hrefs by
                // `/^(https?:|mailto:)/i`, and this is the second, authoritative
                // gate exactly where Python's is; the server-side half of §M8
                // (`check_allowed` inside `h_system_open_path`) belongs to
                // `server.rs`.
                open_external: function(url) {
                    if (!bridgeSafeExternalUrl(url)) { return false; }
                    try {
                        fetch('/api/system/open-path', {
                            method: 'POST',
                            headers: bridgeHeaders(true),
                            body: JSON.stringify({ path: url })
                        }).catch(() => {});
                        return true;
                    } catch(e) { return false; }
                },
                reveal_path: function(path) {
                    fetch('/api/system/reveal-path', {
                        method: 'POST',
                        headers: bridgeHeaders(true),
                        body: JSON.stringify({ path: path })
                    }).catch(() => {});
                },
                open_dir: function(path) {
                    fetch('/api/system/open-path', {
                        method: 'POST',
                        headers: bridgeHeaders(true),
                        body: JSON.stringify({ path: path })
                    }).catch(() => {});
                },
                report_ready: function() {
                    // `Api.report_ready()` is the precise `page_loaded`
                    // milestone; the kernel bridge has no Python object to call,
                    // so it posts a wry IPC message instead.
                    try { window.ipc.postMessage('readmd:page-ready'); } catch(e) {}
                },
                // `Api.show_window()` (`readmd.py:5972`) is `window.show()` +
                // `window.restore()` and always returns `True`; the front end
                // awaits it while handing an external file to an existing window
                // (`assets/js/reader/render.js:249-250`).  The kernel's event loop
                // consumes `readmd:show-window` as
                // `set_visible(true)`+`set_minimized(false)`+`set_focus()`.
                show_window: function() {
                    try { window.ipc.postMessage('readmd:show-window'); } catch(e) {}
                    return true;
                },
                // `Api.toggle_native_fullscreen()` (`readmd.py:5981`) returns
                // `{ok, supported, fullscreen}` -- and the caller only takes the
                // native path when `native.supported` is truthy
                // (`assets/js/reader/render.js:2550-2552`) -- or
                // `{ok:false, code:'native_fullscreen_failed', supported:false}`
                // when the host flip raises.  `window_not_ready` /
                // `native_fullscreen_unavailable` cannot happen here: this script
                // is injected into the only window, and tao implements
                // `set_fullscreen`.
                toggle_native_fullscreen: function() {
                    nativeFullscreen = !nativeFullscreen;
                    try {
                        window.ipc.postMessage(nativeFullscreen ? 'readmd:fullscreen-on' : 'readmd:fullscreen-off');
                        return { ok: true, supported: true, fullscreen: nativeFullscreen };
                    } catch(e) {
                        nativeFullscreen = !nativeFullscreen;
                        return { ok: false, code: 'native_fullscreen_failed', supported: false };
                    }
                },
                request_quit: function() {
                    // `Api.request_quit()` -> `quit_app()` (`readmd.py:460-474`).
                    // There is no `/api/system/quit` route in the kernel, so the
                    // old fetch() always 404'd; the IPC message reaches the event
                    // loop, which runs the shutdown tail and exits.
                    try { window.ipc.postMessage('readmd:quit'); } catch(e) {}
                }
            }
            };
        })();
    "#;

    // `get_app_info` used to return hard-coded '2.4.0' / 'windows'.  Both values
    // have a single source of truth in the kernel, so splice them in here, and
    // `@@READMD_PROBE_MODE@@` carries Python's `_STARTUP_PROBE['enabled']`
    // (`readmd.py:4996`, `readmd.py:1540`) into the updater shim.
    let bridge_init_script = bridge_init_script
        .replace("@@READMD_VERSION@@", server::VERSION)
        .replace("@@READMD_PLATFORM@@", std::env::consts::OS)
        .replace("@@READMD_PROBE_MODE@@", if probe.is_some() { "true" } else { "false" });

    let webview_dir = data_dir.join("webview");
    let _ = std::fs::create_dir_all(&webview_dir);
    let mut web_context = WebContext::new(Some(webview_dir));

    // `window.events.loaded += _on_loaded` (`readmd.py:6572-6584`) is the
    // `window_loaded` milestone; wry reports the same moment as
    // `PageLoadEvent::Finished`.  `Api.report_ready()` and `Api.request_quit()`
    // have no Python object to call in the kernel, so the bridge posts wry IPC
    // messages instead and the flags below are the receiving end.
    let loaded_flag = Arc::new(AtomicBool::new(false));
    let page_ready_flag = Arc::new(AtomicBool::new(false));
    let quit_flag = Arc::new(AtomicBool::new(false));
    let show_flag = Arc::new(AtomicBool::new(false));
    let fullscreen_flag = Arc::new(AtomicUsize::new(FULLSCREEN_IDLE));
    let shared_url = url.clone();
    // The first attempt and the isolated-profile retry must build exactly the same
    // webview, so the builder comes from one factory function.
    let builder = build_webview(
        &mut web_context,
        &shared_url,
        &bridge_init_script,
        loaded_flag.clone(),
        page_ready_flag.clone(),
        quit_flag.clone(),
        show_flag.clone(),
        fullscreen_flag.clone(),
    );
    #[cfg(not(target_os = "linux"))]
    let webview = builder.build(&window);
    #[cfg(target_os = "linux")]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        match window.default_vbox() {
            Some(vbox) => builder.build_gtk(vbox),
            None => Err(wry::Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "no gtk vbox",
            ))),
        }
    };

    // Retry with isolated temp profile if default webview dir had a lock conflict
    let mut retry_profile: Option<PathBuf> = None;
    let webview = match webview {
        Ok(wv) => Ok(wv),
        Err(err) => {
            eprintln!("readmd: webview attempt 1 failed ({err}), retrying with isolated profile...");
            let fb_dir = std::env::temp_dir().join(format!("readmd_wv_{}", std::process::id()));
            let _ = std::fs::create_dir_all(&fb_dir);
            retry_profile = Some(fb_dir.clone());
            let mut fb_context = WebContext::new(Some(fb_dir));
            let fb_builder = build_webview(
                &mut fb_context,
                &shared_url,
                &bridge_init_script,
                loaded_flag.clone(),
                page_ready_flag.clone(),
                quit_flag.clone(),
                show_flag.clone(),
                fullscreen_flag.clone(),
            );
            #[cfg(not(target_os = "linux"))]
            let res = fb_builder.build(&window);
            #[cfg(target_os = "linux")]
            let res = Err(wry::Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "linux retry not supported",
            )));
            res
        }
    };

    let mut webview = match webview {
        Ok(webview) => {
            window.set_visible(true);
            window.set_focus();
            Some(webview)
        }
        Err(err) => {
            eprintln!("readmd: webview failed ({err}), launching browser app");
            drop(window);
            launch_browser_app(&url);
            if let Some(dir) = retry_profile {
                let _ = std::fs::remove_dir_all(dir);
            }
            // Python's non-native fallback also ends in the shared tail
            // (`readmd.py:6662-6675` then `6697-6709`), i.e. a normal exit.
            static FALLBACK_ONCE: AtomicBool = AtomicBool::new(false);
            exit(shutdown(&FALLBACK_ONCE, &data_dir, &probe, &probe_json, false, false));
        }
    };

    let deadline = probe.as_ref().map(|probe| {
        let timeout = probe.lock().unwrap_or_else(|e| e.into_inner()).timeout;
        Instant::now() + timeout
    });
    let done = Arc::new(AtomicBool::new(false));
    let mut next_poll = Instant::now();
    let mut window_handle = Some(window);
    event_loop.run(move |event, _target, control_flow| {
        if done.load(Ordering::SeqCst) {
            // `ControlFlow::Exit*` is sticky; stay out of the way.
            *control_flow = ControlFlow::Exit;
            return;
        }
        *control_flow = ControlFlow::Wait;
        let closing = matches!(
            event,
            Event::WindowEvent { event: WindowEvent::CloseRequested { .. }, .. }
        );
        if !closing && !matches!(event, Event::MainEventsCleared) {
            return;
        }
        // `Api.show_window()` (`readmd.py:5972`) is `window.show()` +
        // `window.restore()`.  Python reaches it from the tray and from the
        // double-click handoff; the front end calls it from `openExternalFile`
        // (`assets/js/reader/render.js:249`).  The kernel has no tray, so this is
        // the only path that can bring a hidden window back.
        if show_flag.swap(false, Ordering::SeqCst) {
            if let Some(window) = window_handle.as_ref() {
                window.set_visible(true);
                window.set_minimized(false);
                window.set_focus();
            }
        }
        // `Api.toggle_native_fullscreen()` (`readmd.py:5981`) returns
        // `{ok, supported, fullscreen}`; the bridge computes the wanted state and
        // posts it, so the kernel only applies it to the real window.
        match fullscreen_flag.swap(FULLSCREEN_IDLE, Ordering::SeqCst) {
            FULLSCREEN_ENTER => {
                if let Some(window) = window_handle.as_ref() {
                    window.set_fullscreen(Some(tao::window::Fullscreen::Borderless(None)));
                }
            }
            FULLSCREEN_EXIT => {
                if let Some(window) = window_handle.as_ref() {
                    window.set_fullscreen(None);
                }
            }
            _ => {}
        }
        if loaded_flag.load(Ordering::SeqCst) {
            probe_mark(&probe, "window_loaded");
        }
        let ready = page_ready_flag.load(Ordering::SeqCst) || quit_flag.load(Ordering::SeqCst);
        if ready && !quit_flag.load(Ordering::SeqCst) {
            probe_mark(&probe, "page_loaded");
        }
        let mut timed_out = false;
        if let Some(at) = deadline {
            if Instant::now() >= at && !ready {
                timed_out = true;
            }
        }
        // Outside a probe run the window *is* the application, so only a real
        // close or a `quit_app()` ends it.  `_on_closing` (`readmd.py:6587-6601`)
        // hands the close back to the platform whenever the page never reported
        // ready, because hiding without a tray leaves an unreachable app.
        let ending = closing || quit_flag.load(Ordering::SeqCst) || deadline.is_some() && (ready || timed_out);
        if !ending {
            if let Some(at) = deadline {
                if Instant::now() >= next_poll {
                    next_poll = Instant::now() + Duration::from_millis(120);
                    // `_probe_fallback` (`readmd.py:6575-6579`): the probe must not
                    // wait for a page that never calls `report_ready()`, so poll the
                    // same condition Python's fallback asserts on.
                    if let Some(webview) = webview.as_ref() {
                        let sink = page_ready_flag.clone();
                        let _ = webview.evaluate_script_with_callback(
                            "document.readyState",
                            move |value: String| {
                                if value.contains("complete") {
                                    sink.store(true, Ordering::SeqCst);
                                }
                            },
                        );
                    }
                }
                *control_flow = ControlFlow::WaitUntil(next_poll.min(at));
            }
            return;
        }
        drop(webview.take());
        drop(window_handle.take());
        if let Some(dir) = retry_profile.take() {
            let _ = std::fs::remove_dir_all(dir);
        }
        let code = shutdown(&done, &data_dir, &probe, &probe_json, timed_out, false);
        *control_flow = ControlFlow::ExitWithCode(code);
    });
}

/// `readmd.py:6502-6508`: `--share` turns the LAN share on straight after boot.
/// The kernel already serves that behaviour on its own loopback socket, so the
/// flag drives `/api/share/start` and prints the same two answers.
fn enable_share(port: u16) {
    let body = loopback_body(
        port,
        "POST",
        "/api/share/start",
        Some("{}"),
        Duration::from_secs(5),
    );
    if json_field(&body, "ok").as_deref() == Some("true") {
        println!("局域网共享已开启：{}", json_field(&body, "url").unwrap_or_default());
    } else {
        let error = json_field(&body, "error")
            .or_else(|| json_field(&body, "detail"))
            .unwrap_or_else(|| "unknown".to_string());
        println!("局域网共享失败：{}", error);
    }
}

fn main() {
    #[cfg(target_os = "windows")]
    attach_console();

    let opts = match parse_args() {
        Ok(o) => o,
        Err(err) => {
            // `parser.error()` (`readmd.py:6419`, `6438-6443`): usage plus the
            // message on stderr, exit status 2.
            eprint!("{}", USAGE);
            eprintln!("readmd: error: {}", err);
            exit(2);
        }
    };

    // `readmd.py:6421-6436`: a diagnosis prints its report and returns before
    // logging, the store or the server are touched.
    if let Some(target) = opts.check {
        println!("{}", diagnose_report(target));
        exit(0);
    }

    // `_T0 = time.time()` and the milestone reset (`readmd.py:6451-6457`) happen
    // before the one-shot modes, so a boot clock exists even if nothing else runs.
    let probe: Option<Probe> = if opts.startup_probe {
        // `readmd.py:6455` — `_STARTUP_PROBE.update({'enabled': True, ...})`.  This
        // is the single writer of the flag every later `_STARTUP_PROBE.get('enabled')`
        // read answers on, so arming the probe clock without arming the shared flag
        // would let the startup check do real GitHub egress.
        readmd_kernel::batch2::set_startup_probe_mode(true);
        Some(Arc::new(Mutex::new(ProbeInner {
            started: Instant::now(),
            timeout: Duration::from_secs_f64(opts.startup_probe_timeout),
            milestones: Vec::new(),
            timed_out: false,
        })))
    } else {
        None
    };

    let paths = match resolve_paths(&opts) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("readmd: cannot resolve paths ({})", err);
            exit(1);
        }
    };

    // `--assoc` (`readmd.py:6459-6462`): only `APP_DIR`/`DATA_DIR` are needed, so
    // it must not bootstrap the store.
    if opts.assoc {
        let (ok, message) = associate_markdown(&paths);
        println!("association: {}", message);
        exit(if ok { 0 } else { 1 });
    }

    let app = match App::bootstrap(paths) {
        Ok(a) => Arc::new(a),
        Err(err) => {
            eprintln!("readmd: bootstrap failed ({})", err);
            exit(1);
        }
    };

    if opts.selftest {
        exit(run_selftest(app.clone()));
    }
    if opts.webview_selftest {
        exit(run_webview_selftest(app.clone()));
    }
    if opts.mods {
        exit(run_mods(app.clone()));
    }

    // ------------------------------------------------------ single instance
    //
    // `alive = None if args.startup_probe else instance_alive()`
    // (`readmd.py:6486-6491`): a probe run always owns its own instance, so it
    // never forwards.  A second launch that finds a live instance hands over its
    // file and exits 0 — that is what makes a double-clicked `.md` open instantly.
    let alive = if opts.startup_probe {
        None
    } else {
        instance_alive(&app.paths.data_dir)
    };
    if let Some((port, token)) = alive {
        match &opts.file {
            // `if not args.file or forward_open(...)`: with no document there is
            // nothing to forward and the resident window is left alone.
            None => exit(0),
            Some(raw) => {
                // `os.path.abspath(args.file)` — forwarded even when the file is
                // missing; the resident instance reports that, not this process.
                if forward_open(port, &token, &abspath(raw)) {
                    exit(0);
                }
            }
        }
    }

    // --------------------------------------------------------------- bind
    //
    // `start_server(args.port, args.host)` (`readmd.py:3685-3703`): the default is
    // the fixed control port, because `instance.json` — and therefore single
    // instance — only exists while this process owns 26891.  A foreign occupant
    // falls back to an ephemeral port and silently gives up single instance.
    // `if not port: port = CONTROL_PORT` is Python's zero-means-default rule, and
    // `HTTPServer((host, port))` raises `OSError` for a port it cannot represent
    // (`-1`, `99999`) just as it does for a busy one, so both take the same
    // ephemeral retry rather than an argument error.
    let want: i64 = if opts.port == 0 {
        server::CONTROL_PORT as i64
    } else {
        opts.port
    };
    let port = match u16::try_from(want) {
        Err(_) => server::spawn(app.clone(), &opts.host, 0),
        Ok(want) => match server::spawn(app.clone(), &opts.host, want) {
            Ok(bound) => Ok(bound),
            // `except OSError: server = ReadMDHTTPServer((bind_host, 0), Handler)`.
            Err(_) => server::spawn(app.clone(), &opts.host, 0),
        },
    }
    .map_err(|err| (want, err));
    let port = match port {
        Ok(port) => port,
        Err((want, err)) => {
            eprintln!("readmd: cannot bind {}:{} ({})", opts.host, want, err);
            exit(1);
        }
    };
    // `milestone('boot', 'server_up')` (`readmd.py:6496`).
    probe_mark(&probe, "server_up");

    println!("ReadMD rust kernel {} listening on http://{}:{}", server::VERSION, opts.host, port);
    println!("  data      {}", app.paths.data_dir.display());
    println!("  workspace {}", app.paths.workspace.display());
    println!("  assets    {}", app.paths.assets_dir.display());
    if opts.print_token {
        // Python never prints the secret; `readmd.py:6518` only prints the URL.
        println!("  token     {}", app.app_token);
    }

    if opts.share {
        enable_share(port);
    }

    let initial = resolve_initial(opts.file.as_ref());
    // `display_host = '127.0.0.1' if args.host in ('0.0.0.0', '::') else args.host`
    // and `url += '?file=' + quote(initial)` (`readmd.py:6517-6520`).
    let view_host = if opts.host == "0.0.0.0" || opts.host == "::" {
        "127.0.0.1".to_string()
    } else {
        opts.host.clone()
    };
    let mut url = format!("http://{}:{}/", view_host, port);
    if let Some(doc) = &initial {
        url.push_str("?file=");
        url.push_str(&url_quote(doc));
    }

    if opts.browser {
        // `webbrowser.open(url)`, the Ctrl+C park, `_clear_instance()`
        // (`readmd.py:6522-6531`).
        open_in_default_browser(&url);
        println!("ReadMD 服务运行于 {}（Ctrl+C 退出）", url);
        park_until_quit();
        static BROWSER_TAIL: AtomicBool = AtomicBool::new(false);
        exit(shutdown(&BROWSER_TAIL, &app.paths.data_dir, &probe, &opts.startup_probe_json, false, false));
    }

    if !opts.window {
        // Kernel-only headless mode (`--no-window`): serve, but open neither
        // window nor browser.  The park is interruptible, unlike the old
        // `sleep(600)` loop, and the tail removes `instance.json`.
        park_until_quit();
        static HEADLESS_TAIL: AtomicBool = AtomicBool::new(false);
        exit(shutdown(&HEADLESS_TAIL, &app.paths.data_dir, &probe, &opts.startup_probe_json, false, false));
    }

    // `run_window` never returns: tao's `EventLoop::run` is `-> !` and exits the
    // process with the code its handler leaves in `ControlFlow`.
    run_window(url, app.paths.data_dir.clone(), probe, opts.startup_probe_json.clone());
}
