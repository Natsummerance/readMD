// -*- mode: rust; coding: utf-8 -*-
//! ReadMD High-Precision LaTeX <-> Markdown Bidirectional Conversion Engine.
//!
//! Native Rust port of `src/readmd_modules/texmd.py` (1523 lines).  Pure `std`,
//! zero external dependencies — including a hand-written backtracking regular
//! expression engine, because the Python module is *specified* in terms of
//! CPython `re` semantics (leftmost-first backtracking, lazy quantifiers,
//! back-references, fixed-width look-behind and capture-group aware `re.split`).
//!
//! Parity notes that apply to the whole file:
//! * Text is handled as `Vec<char>` (code points) inside the engine so every
//!   Python `str` index/slice maps 1:1 and saturates like CPython instead of
//!   panicking on a byte offset.
//! * `py_isspace` / `py_strip` / `py_splitlines` reproduce the *CPython*
//!   whitespace set, which is wider than Rust's `char::is_whitespace`
//!   (U+001C..U+001F) and breaks lines on more characters than `str::lines`.
//! * `texmd.py` has no deep recursion (only `_expand_inputs`, capped at
//!   `depth > 10`), so the port keeps that recursion and its cap.  The regex
//!   matcher does *not* mirror CPython's recursive `sre` backtracker: it is an
//!   explicit-stack VM that keeps its continuation stack on the heap, so its
//!   memory footprint is independent of native stack size and deeply nested or
//!   very long math spans cannot overflow the stack.  Its only bound is the
//!   backtracking step budget, which mirrors CPython's "give up" behaviour
//!   rather than crashing.
//! * `latex_to_md()` runs document metadata through
//!   `plugin_runtime.latex_label()`, which is an *optional plugin* bridge:
//!   `run_plugin('pylatexenc', ..., default=text)` returns its input untouched
//!   whenever `pylatexenc` is not enabled/installed.  `plugin_manager.rs`
//!   asserts `pylatexenc` is not in `DEFAULT_ENABLED`, so this port implements
//!   the disabled-plugin path (identity).  See [`latex_label`].

#![allow(dead_code)]

use std::vec::Vec;

// ===========================================================================
// 0. CPython-compatible text helpers
// ===========================================================================

/// CPython `str.isspace()` character set.  Rust's `char::is_whitespace` misses
/// U+001C..U+001F, which CPython counts as whitespace.
#[inline]
pub fn py_isspace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Characters `str.splitlines()` breaks on (beyond `\n`, `\r\n`, `\r`).
#[inline]
fn py_linebreak(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r' | '\u{0b}' | '\u{0c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}'
            | '\u{2028}' | '\u{2029}'
    )
}

/// `text.splitlines()` — line terminators dropped, as in CPython.
pub fn py_splitlines(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    py_splitlines_c(&chars)
        .into_iter()
        .map(|v| chars_to_string(&v))
        .collect()
}

pub fn py_splitlines_c(v: &[char]) -> Vec<Vec<char>> {
    let mut out: Vec<Vec<char>> = Vec::new();
    let mut cur: Vec<char> = Vec::new();
    let mut i = 0usize;
    while i < v.len() {
        let c = v[i];
        if py_linebreak(c) {
            let mut n = 1usize;
            if c == '\r' && i + 1 < v.len() && v[i + 1] == '\n' {
                n = 2;
            }
            out.push(std::mem::take(&mut cur));
            i += n;
            continue;
        }
        cur.push(c);
        i += 1;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[inline]
pub fn to_chars(text: &str) -> Vec<char> {
    text.chars().collect()
}

#[inline]
pub fn chars_to_string(v: &[char]) -> String {
    v.iter().collect()
}

/// `text.strip()` with the CPython whitespace set.
pub fn py_strip(text: &str) -> String {
    chars_to_string(&py_strip_chars(&to_chars(text)))
}

pub fn py_strip_chars(v: &[char]) -> Vec<char> {
    let mut a = 0usize;
    while a < v.len() && py_isspace(v[a]) {
        a += 1;
    }
    let mut b = v.len();
    while b > a && py_isspace(v[b - 1]) {
        b -= 1;
    }
    v[a..b].to_vec()
}

pub fn py_lstrip_chars(v: &[char]) -> Vec<char> {
    let mut a = 0usize;
    while a < v.len() && py_isspace(v[a]) {
        a += 1;
    }
    v[a..].to_vec()
}

pub fn py_rstrip_chars(v: &[char]) -> Vec<char> {
    let mut b = v.len();
    while b > 0 && py_isspace(v[b - 1]) {
        b -= 1;
    }
    v[..b].to_vec()
}

/// `x.strip(chars)` / `x.rstrip(chars)` with an explicit character predicate.
pub fn py_strip_set(v: &[char], set: &dyn Fn(char) -> bool) -> Vec<char> {
    let mut a = 0usize;
    while a < v.len() && set(v[a]) {
        a += 1;
    }
    let mut b = v.len();
    while b > a && set(v[b - 1]) {
        b -= 1;
    }
    v[a..b].to_vec()
}

pub fn py_rstrip_set(v: &[char], set: &dyn Fn(char) -> bool) -> Vec<char> {
    let mut b = v.len();
    while b > 0 && set(v[b - 1]) {
        b -= 1;
    }
    v[..b].to_vec()
}

/// Python truthiness of a `str` (`if not x:` is true for the empty string).
#[inline]
pub fn py_truthy(s: &str) -> bool {
    !s.is_empty()
}

/// `v[a:b]` with Python's saturating, never-panicking slice semantics.
pub fn py_slice(v: &[char], start: usize, end: usize) -> Vec<char> {
    let a = start.min(v.len());
    let b = end.min(v.len());
    if b <= a {
        Vec::new()
    } else {
        v[a..b].to_vec()
    }
}

pub fn starts_with(v: &[char], prefix: &str) -> bool {
    let p: Vec<char> = prefix.chars().collect();
    if p.len() > v.len() {
        return false;
    }
    v[..p.len()] == p[..]
}

pub fn ends_with(v: &[char], suffix: &str) -> bool {
    let p: Vec<char> = suffix.chars().collect();
    if p.len() > v.len() {
        return false;
    }
    v[v.len() - p.len()..] == p[..]
}

pub fn contains_str(v: &[char], needle: &str) -> bool {
    let p: Vec<char> = needle.chars().collect();
    find_sub(v, &p, 0).is_some()
}

/// `text.find(needle, from)`; `None` == Python's `-1`.
pub fn find_sub(hay: &[char], needle: &[char], start: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(start.min(hay.len()));
    }
    if start >= hay.len() {
        return None;
    }
    let mut i = start;
    while i + needle.len() <= hay.len() {
        if hay[i..i + needle.len()] == *needle {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// `text.split(sep)` on a code-point buffer (keeps empty fields).
pub fn py_split_c(v: &[char], sep: char) -> Vec<Vec<char>> {
    let mut out: Vec<Vec<char>> = Vec::new();
    let mut cur: Vec<char> = Vec::new();
    for c in v {
        if *c == sep {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(*c);
        }
    }
    out.push(cur);
    out
}

/// `text.replace(old, new)` for every occurrence.
pub fn py_replace_c(v: &[char], old: &str, new: &str) -> Vec<char> {
    let o: Vec<char> = old.chars().collect();
    if o.is_empty() {
        // CPython inserts `new` between every character.
        let n: Vec<char> = new.chars().collect();
        let mut out: Vec<char> = Vec::new();
        for (i, c) in v.iter().enumerate() {
            if i > 0 {
                out.extend_from_slice(&n);
            }
            out.push(*c);
        }
        if !v.is_empty() {
            out.extend_from_slice(&n);
        } else {
            out.extend_from_slice(&n);
        }
        return out;
    }
    let mut out: Vec<char> = Vec::new();
    let mut i = 0usize;
    while i < v.len() {
        if i + o.len() <= v.len() && v[i..i + o.len()] == o[..] {
            out.extend(new.chars());
            i += o.len();
        } else {
            out.push(v[i]);
            i += 1;
        }
    }
    out
}

pub fn py_replace(text: &str, old: &str, new: &str) -> String {
    chars_to_string(&py_replace_c(&to_chars(text), old, new))
}

/// CPython 3.7+ `re.escape()`: only special characters are backslash-escaped.
pub fn re_escape(text: &str) -> String {
    const SPECIAL: &str = "()[]{}?*+-|^$\\.&~# \t\n\r\u{0b}\u{0c}";
    let mut out = String::new();
    for c in text.chars() {
        if SPECIAL.contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

// ===========================================================================
// 1. std-only backtracking regex engine (CPython `re` semantics)
// ===========================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Fl {
    pub dotall: bool,
    pub ignore_case: bool,
    pub multiline: bool,
}

impl Fl {
    pub const NONE: Fl = Fl { dotall: false, ignore_case: false, multiline: false };
    pub const DOTALL: Fl = Fl { dotall: true, ignore_case: false, multiline: false };
    pub const IC: Fl = Fl { dotall: false, ignore_case: true, multiline: false };
    pub const DOTALL_IC: Fl = Fl { dotall: true, ignore_case: true, multiline: false };
}

#[derive(Clone, Debug)]
enum CItem {
    Ch(char),
    Range(char, char),
    Cls(char),
}

#[derive(Clone, Debug)]
struct ClassDef {
    neg: bool,
    items: Vec<CItem>,
    ic: bool,
}

/// Instruction stream.  All jump operands are *relative* signed offsets so that
/// compiled blocks can be spliced without back-patching.
#[derive(Clone, Debug)]
enum In {
    Char(char),
    CharI(char),
    Any,
    AnyX,
    Class(usize),
    Start,
    End,
    EndOptNl,
    StartStr,
    EndStr,
    WordB,
    NotWordB,
    Save(usize),
    Split(isize, isize),
    Jmp(isize),
    SplitLoop { body: isize, after: isize, slot: usize, body_first: bool },
    JmpLoop { head: isize, after: isize, slot: usize },
    Backref(usize),
    Look { neg: bool, behind: bool, prog: Vec<In>, width: Option<usize> },
    Match,
}

#[derive(Clone, Debug)]
enum Node {
    Char(char),
    Any,
    Class(usize),
    Start,
    End,
    EndOptNl,
    StartStr,
    EndStr,
    WordB,
    NotWordB,
    Group { idx: Option<usize>, alts: Vec<Vec<Node>> },
    Alt(Vec<Vec<Node>>),
    Rep { body: Vec<Node>, min: usize, max: Option<usize>, greedy: bool },
    Look { neg: bool, behind: bool, body: Vec<Node> },
    Backref(usize),
}

const UNSET: usize = usize::MAX;

/// A match: code-point offsets plus capture slots (slot 0/1 == group 0).
#[derive(Clone, Debug)]
pub struct Mx {
    pub start: usize,
    pub end: usize,
    caps: Vec<usize>,
}

impl Mx {
    /// `m.group(i)` — `None` when the group did not participate (Python `None`).
    pub fn grp(&self, s: &[char], i: usize) -> Option<Vec<char>> {
        let a = *self.caps.get(2 * i)?;
        let b = *self.caps.get(2 * i + 1)?;
        if a == UNSET || b == UNSET {
            return None;
        }
        Some(py_slice(s, a, b))
    }
    pub fn gs(&self, s: &[char], i: usize) -> Option<String> {
        self.grp(s, i).map(|v| chars_to_string(&v))
    }
    /// `m.group(i)` coerced to `''` (the Python `m.group(1) or ''` idiom is
    /// spelled out at the call sites that need the distinction).
    pub fn gs_or_empty(&self, s: &[char], i: usize) -> String {
        self.gs(s, i).unwrap_or_default()
    }
    pub fn start_(&self) -> usize {
        self.start
    }
    pub fn end_(&self) -> usize {
        self.end
    }
}

#[inline]
fn c_lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

#[inline]
fn eq_ic(a: char, b: char) -> bool {
    a == b || c_lower(a) == c_lower(b)
}

/// Python `\w` for `str` patterns: Unicode alphanumerics plus underscore.
#[inline]
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Python `\d` for `str` patterns: Unicode decimal digits.  `char::is_numeric`
/// is slightly wider (Nl/No) because `std` has no Nd predicate.
#[inline]
fn is_digit(c: char) -> bool {
    c.is_numeric()
}

fn class_match(cd: &ClassDef, c: char) -> bool {
    let mut hit = false;
    for it in &cd.items {
        let m = match it {
            CItem::Ch(x) => {
                if cd.ic {
                    eq_ic(*x, c)
                } else {
                    *x == c
                }
            }
            CItem::Range(a, b) => {
                if !cd.ic {
                    c >= *a && c <= *b
                } else {
                    in_range_ic(c, *a, *b)
                }
            }
            CItem::Cls(k) => match *k {
                's' => py_isspace(c),
                'S' => !py_isspace(c),
                'd' => is_digit(c),
                'D' => !is_digit(c),
                'w' => is_word(c),
                'W' => !is_word(c),
                _ => false,
            },
        };
        if m {
            hit = true;
            break;
        }
    }
    if cd.neg {
        !hit
    } else {
        hit
    }
}

fn in_range_ic(c: char, a: char, b: char) -> bool {
    let cands = [c, c_lower(c), c.to_uppercase().next().unwrap_or(c)];
    let lows = [a, c_lower(a)];
    let highs = [b, b.to_uppercase().next().unwrap_or(b)];
    for x in cands.iter() {
        for l in lows.iter() {
            for h in highs.iter() {
                if x >= l && x <= h {
                    return true;
                }
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// parser
// ---------------------------------------------------------------------------

struct Parser<'a> {
    s: &'a [char],
    p: usize,
    n_groups: usize,
    flags: Fl,
    classes: Vec<ClassDef>,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.p).copied()
    }
    fn peek2(&self) -> Option<char> {
        self.s.get(self.p + 1).copied()
    }
    fn eat(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.p += 1;
        }
        c
    }
    fn push_class(&mut self, cd: ClassDef) -> usize {
        self.classes.push(cd);
        self.classes.len() - 1
    }

    fn parse_alt(&mut self) -> Vec<Vec<Node>> {
        let mut alts: Vec<Vec<Node>> = Vec::new();
        let mut cur: Vec<Node> = Vec::new();
        loop {
            match self.peek() {
                None | Some(')') => break,
                Some('|') => {
                    self.p += 1;
                    alts.push(std::mem::take(&mut cur));
                }
                Some(_) => {
                    if let Some(n) = self.parse_piece() {
                        cur.push(n);
                    }
                }
            }
        }
        alts.push(cur);
        alts
    }

    /// One atom plus its quantifier.
    fn parse_piece(&mut self) -> Option<Node> {
        let atom = self.parse_atom()?;
        let (min, max, greedy) = match self.peek() {
            Some('*') => {
                self.p += 1;
                if self.peek() == Some('?') {
                    self.p += 1;
                    (0, None, false)
                } else {
                    (0, None, true)
                }
            }
            Some('+') => {
                self.p += 1;
                if self.peek() == Some('?') {
                    self.p += 1;
                    (1, None, false)
                } else {
                    (1, None, true)
                }
            }
            Some('?') => {
                self.p += 1;
                if self.peek() == Some('?') {
                    self.p += 1;
                    (0, Some(1), false)
                } else {
                    (0, Some(1), true)
                }
            }
            Some('{') => match self.try_counted_repeat() {
                Some(rep) => rep,
                None => return Some(atom),
            },
            _ => return Some(atom),
        };
        Some(Node::Rep { body: vec![atom], min, max, greedy })
    }

    fn try_counted_repeat(&mut self) -> Option<(usize, Option<usize>, bool)> {
        let save = self.p;
        self.p += 1; // '{'
        let mut ds = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                ds.push(c);
                self.p += 1;
            } else {
                break;
            }
        }
        if ds.is_empty() {
            self.p = save;
            return None;
        }
        let min: usize = ds.parse().unwrap_or(0);
        let max = if self.peek() == Some(',') {
            self.p += 1;
            let mut ds2 = String::new();
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    ds2.push(c);
                    self.p += 1;
                } else {
                    break;
                }
            }
            if ds2.is_empty() {
                None
            } else {
                Some(ds2.parse::<usize>().unwrap_or(min))
            }
        } else {
            Some(min)
        };
        if self.peek() != Some('}') {
            self.p = save;
            return None;
        }
        self.p += 1;
        let greedy = if self.peek() == Some('?') {
            self.p += 1;
            false
        } else {
            true
        };
        Some((min, max, greedy))
    }

    fn parse_atom(&mut self) -> Option<Node> {
        let c = self.eat()?;
        match c {
            '.' => Some(Node::Any),
            '^' => {
                if self.flags.multiline {
                    Some(Node::Start)
                } else {
                    Some(Node::StartStr)
                }
            }
            '$' => {
                if self.flags.multiline {
                    Some(Node::End)
                } else {
                    Some(Node::EndOptNl)
                }
            }
            '[' => Some(Node::Class(self.parse_class())),
            '(' => self.parse_group(),
            '\\' => Some(self.parse_escape(false)),
            other => Some(Node::Char(other)),
        }
    }

    fn parse_escape(&mut self, in_class: bool) -> Node {
        let c = match self.eat() {
            Some(c) => c,
            None => return Node::Char('\\'),
        };
        match c {
            's' | 'd' | 'w' | 'S' | 'D' | 'W' => {
                let idx = self.push_class(ClassDef {
                    neg: matches!(c, 'S' | 'D' | 'W'),
                    items: vec![CItem::Cls(c.to_ascii_lowercase())],
                    ic: self.flags.ignore_case,
                });
                Node::Class(idx)
            }
            'b' if !in_class => Node::WordB,
            'B' if !in_class => Node::NotWordB,
            'A' => Node::StartStr,
            'Z' | 'z' => Node::EndStr,
            'n' => Node::Char('\n'),
            't' => Node::Char('\t'),
            'r' => Node::Char('\r'),
            'v' => Node::Char('\u{0b}'),
            'f' => Node::Char('\u{0c}'),
            'a' => Node::Char('\u{07}'),
            'x' | 'u' | 'U' => {
                let n = match c {
                    'x' => 2,
                    'u' => 4,
                    _ => 8,
                };
                let mut v = String::new();
                for _ in 0..n {
                    match self.peek() {
                        Some(h) if h.is_ascii_hexdigit() => {
                            v.push(h);
                            self.p += 1;
                        }
                        _ => break,
                    }
                }
                if v.is_empty() {
                    return Node::Char(c);
                }
                let cp = u32::from_str_radix(&v, 16).unwrap_or(0);
                Node::Char(char::from_u32(cp).unwrap_or('\u{fffd}'))
            }
            '0'..='9' if !in_class => {
                let mut num = (c as u32 - '0' as u32) as usize;
                while let Some(d) = self.peek() {
                    if !d.is_ascii_digit() {
                        break;
                    }
                    let cand = num * 10 + (d as u32 - '0' as u32) as usize;
                    if cand > self.n_groups.max(9) {
                        break;
                    }
                    num = cand;
                    self.p += 1;
                }
                Node::Backref(num)
            }
            other => Node::Char(other),
        }
    }

    fn parse_class(&mut self) -> usize {
        let neg = self.peek() == Some('^');
        if neg {
            self.p += 1;
        }
        let mut items: Vec<CItem> = Vec::new();
        let mut first = true;
        loop {
            let c = match self.peek() {
                None => break,
                Some(']') if !first => {
                    self.p += 1;
                    break;
                }
                Some(x) => x,
            };
            first = false;
            self.p += 1;
            if c == '\\' {
                match self.eat() {
                    None => break,
                    Some(k @ ('s' | 'S' | 'd' | 'D' | 'w' | 'W')) => {
                        items.push(CItem::Cls(k));
                        continue;
                    }
                    Some(other) => {
                        let ch = self.unescape_char(other);
                        if self.push_range(&mut items, ch) {
                            continue;
                        }
                    }
                }
            } else {
                if self.push_range(&mut items, c) {
                    continue;
                }
                items.push(CItem::Ch(c));
            }
        }
        self.push_class(ClassDef { neg, items, ic: self.flags.ignore_case })
    }

    /// After having consumed a literal class item `lo`, consume `lo-hi` when a
    /// `-` follows.  Returns true when a Range (or the expanded literals) was
    /// appended, false when the caller still has to push `lo`.
    fn push_range(&mut self, items: &mut Vec<CItem>, lo: char) -> bool {
        if self.peek() != Some('-') {
            items.push(CItem::Ch(lo));
            return true;
        }
        let save = self.p;
        self.p += 1;
        match self.peek() {
            None | Some(']') => {
                self.p = save;
                items.push(CItem::Ch(lo));
                items.push(CItem::Ch('-'));
                true
            }
            Some('-') => {
                // `a--z` == a, -, z
                self.p += 1;
                items.push(CItem::Ch(lo));
                items.push(CItem::Ch('-'));
                true
            }
            Some(_) => {
                let hc = self.eat().unwrap();
                if hc == '\\' {
                    match self.eat() {
                        None => {
                            self.p = save;
                            return false;
                        }
                        Some(k @ ('s' | 'S' | 'd' | 'D' | 'w' | 'W')) => {
                            items.push(CItem::Ch(lo));
                            items.push(CItem::Ch('-'));
                            items.push(CItem::Cls(k));
                            return true;
                        }
                        Some(other) => {
                            let hi = self.unescape_char(other);
                            items.push(CItem::Range(lo, hi));
                            return true;
                        }
                    }
                }
                items.push(CItem::Range(lo, hc));
                true
            }
        }
    }

    fn unescape_char(&self, c: char) -> char {
        match c {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            'v' => '\u{0b}',
            'f' => '\u{0c}',
            'a' => '\u{07}',
            other => other,
        }
    }

    fn parse_group(&mut self) -> Option<Node> {
        if self.peek() == Some('?') {
            let save = self.p;
            self.p += 1;
            match self.peek() {
                Some(':') => {
                    self.p += 1;
                    let alts = self.parse_alt();
                    if self.peek() == Some(')') {
                        self.p += 1;
                    }
                    return Some(Node::Group { idx: None, alts });
                }
                Some('=') | Some('!') => {
                    let neg = self.peek() == Some('!');
                    self.p += 1;
                    let body = join_alt(&self.parse_alt());
                    if self.peek() == Some(')') {
                        self.p += 1;
                    }
                    return Some(Node::Look { neg, behind: false, body });
                }
                Some('<') => {
                    self.p += 1;
                    match self.peek() {
                        Some('=') | Some('!') => {
                            let neg = self.peek() == Some('!');
                            self.p += 1;
                            let body = join_alt(&self.parse_alt());
                            if self.peek() == Some(')') {
                                self.p += 1;
                            }
                            return Some(Node::Look { neg, behind: true, body });
                        }
                        // `(?P<name>...)` / `(?P=name)`: name groups are not
                        // used by texmd.py; treat `(?P<...` as a plain group.
                        _ => {
                            self.p = save;
                            let alts = self.parse_alt();
                            if self.peek() == Some(')') {
                                self.p += 1;
                            }
                            return Some(Node::Group { idx: None, alts });
                        }
                    }
                }
                _ => {
                    self.p = save;
                    let alts = self.parse_alt();
                    if self.peek() == Some(')') {
                        self.p += 1;
                    }
                    return Some(Node::Group { idx: None, alts });
                }
            }
        }
        self.n_groups += 1;
        let idx = self.n_groups;
        let alts = self.parse_alt();
        if self.peek() == Some(')') {
            self.p += 1;
        }
        Some(Node::Group { idx: Some(idx), alts })
    }
}

fn join_alt(alts: &[Vec<Node>]) -> Vec<Node> {
    if alts.len() == 1 {
        return alts[0].clone();
    }
    vec![Node::Alt(alts.to_vec())]
}

struct Compiler {
    n_loops: usize,
    flags: Fl,
}

impl Compiler {
    fn alt_chain(&mut self, alts: &[Vec<Node>]) -> Vec<In> {
        if alts.len() == 1 {
            return self.compile_seq(&alts[0]);
        }
        let head = self.compile_seq(&alts[0]);
        let tail = self.alt_chain(&alts[1..]);
        // [0] Split(1, 2+head.len)  head...  [1+head.len] Jmp(1+tail.len)  tail...
        let mut out: Vec<In> = Vec::with_capacity(2 + head.len() + tail.len());
        out.push(In::Split(1, (2 + head.len()) as isize));
        out.extend(head);
        out.push(In::Jmp(1 + tail.len() as isize));
        out.extend(tail);
        out
    }

    fn compile_seq(&mut self, nodes: &[Node]) -> Vec<In> {
        let mut out: Vec<In> = Vec::new();
        for nd in nodes {
            let block = self.compile_node(nd);
            out.extend(block);
        }
        out
    }

    fn compile_node(&mut self, nd: &Node) -> Vec<In> {
        match nd {
            Node::Char(c) => {
                if self.flags.ignore_case {
                    vec![In::CharI(*c)]
                } else {
                    vec![In::Char(*c)]
                }
            }
            Node::Any => {
                if self.flags.dotall {
                    vec![In::AnyX]
                } else {
                    vec![In::Any]
                }
            }
            Node::Class(i) => vec![In::Class(*i)],
            Node::Start => vec![In::Start],
            Node::End => vec![In::End],
            Node::EndOptNl => vec![In::EndOptNl],
            Node::StartStr => vec![In::StartStr],
            Node::EndStr => vec![In::EndStr],
            Node::WordB => vec![In::WordB],
            Node::NotWordB => vec![In::NotWordB],
            Node::Backref(g) => vec![In::Backref(*g)],
            Node::Alt(alts) => self.alt_chain(alts),
            Node::Group { idx, alts } => {
                let mut out: Vec<In> = Vec::new();
                if let Some(i) = idx {
                    out.push(In::Save(2 * i));
                }
                out.extend(self.alt_chain(alts));
                if let Some(i) = idx {
                    out.push(In::Save(2 * i + 1));
                }
                out
            }
            Node::Look { neg, behind, body } => {
                let prog = self.compile_seq(body);
                let width = if *behind { fixed_width(body) } else { None };
                vec![In::Look { neg: *neg, behind: *behind, prog, width }]
            }
            Node::Rep { body, min, max, greedy } => {
                let mut out: Vec<In> = Vec::new();
                for _ in 0..*min {
                    out.extend(self.compile_seq(body));
                }
                match max {
                    Some(mx) => {
                        let body_c = self.compile_seq(body);
                        for _ in *min..*mx {
                            let mut blk: Vec<In> = Vec::new();
                            blk.push(In::Split(1, (1 + body_c.len()) as isize));
                            blk.extend(body_c.clone());
                            out.extend(blk);
                        }
                    }
                    None => {
                        let slot = self.n_loops;
                        self.n_loops += 1;
                        let body_c = self.compile_seq(body);
                        // [0] SplitLoop [1..] body [1+len] JmpLoop
                        let mut blk: Vec<In> = Vec::new();
                        blk.push(In::SplitLoop {
                            body: 1,
                            after: (2 + body_c.len()) as isize,
                            slot,
                            body_first: *greedy,
                        });
                        blk.extend(body_c);
                        blk.push(In::JmpLoop { head: -(blk.len() as isize), after: 1, slot });
                        out.extend(blk);
                    }
                }
                out
            }
        }
    }
}

/// Width of a look-behind body when it is a plain fixed-length sequence.
fn fixed_width(nodes: &[Node]) -> Option<usize> {
    let mut w = 0usize;
    for n in nodes {
        match n {
            Node::Char(_) | Node::Class(_) | Node::Any => w += 1,
            _ => return None,
        }
    }
    Some(w)
}

/// Compiled pattern.  `texmd.py` relies on CPython's leftmost-first,
/// backtracking semantics, so this is a backtracking VM, not a DFA.
#[derive(Clone, Debug)]
pub struct Re {
    prog: Vec<In>,
    classes: Vec<ClassDef>,
    pub n_groups: usize,
    flags: Fl,
    src: String,
    n_loops: usize,
}

/// One record on the explicit continuation stack of [`Re::m`].
///
/// Every variant maps 1:1 onto a call site that used to be native recursion in
/// that matcher, so the exploration order — and therefore the leftmost-first
/// CPython result, and the number of `budget` steps spent reaching it — is
/// unchanged.
enum MatchFrame<'p> {
    /// `In::Save(k)`: undo the capture slot if the branch below fails.
    Save { ctx: usize, k: usize, old: usize },
    /// `In::Split` and the generic `In::SplitLoop`: the first branch failed,
    /// so try the alternative entry from the same position.
    Alt { ctx: usize, alt: usize, pos: usize },
    /// `In::SplitLoop`: restore the loop's progress slot when the loop's own
    /// result is known.
    Loop { ctx: usize, slot: usize, old: usize },
    /// Bounded single-item `In::SplitLoop`: end positions still to try.
    Ends { ctx: usize, ends: Vec<usize>, i: usize, cont: usize },
    /// `In::Look`: one attempt of the look-around sub-program finished.
    Look {
        ctx: usize,
        neg: bool,
        behind: bool,
        pos: usize,
        after: usize,
        sub: &'p [In],
        widths: Vec<usize>,
        wi: usize,
    },
}

impl<'p> MatchFrame<'p> {
    fn ctx(&self) -> usize {
        match self {
            MatchFrame::Save { ctx, .. }
            | MatchFrame::Alt { ctx, .. }
            | MatchFrame::Loop { ctx, .. }
            | MatchFrame::Ends { ctx, .. }
            | MatchFrame::Look { ctx, .. } => *ctx,
        }
    }
}

/// A position in the program plus the capture/loop slot vectors that belong to
/// it.  Context 0 is the caller's own `caps`/`lp`; deeper contexts belong to
/// look-around sub-programs, which allocate a fresh slot vector per attempt in
/// `texmd.py`'s `re` as well.
struct MatchCtx<'p> {
    prog: &'p [In],
    caps: Vec<usize>,
    lp: Vec<usize>,
}

impl Re {
    pub fn new(pattern: &str, flags: Fl) -> Re {
        let chars: Vec<char> = pattern.chars().collect();
        let mut p = Parser { s: &chars, p: 0, n_groups: 0, flags, classes: Vec::new() };
        let alts = p.parse_alt();
        let classes = std::mem::take(&mut p.classes);
        let n_groups = p.n_groups;
        let mut comp = Compiler { n_loops: 0, flags };
        let mut prog = comp.alt_chain(&alts);
        prog.push(In::Match);
        Re {
            prog,
            classes,
            n_groups,
            flags,
            src: pattern.to_string(),
            n_loops: comp.n_loops,
        }
    }

    pub fn pattern(&self) -> &str {
        &self.src
    }

    fn slots(&self) -> Vec<usize> {
        vec![UNSET; 2 * (self.n_groups + 1)]
    }

    /// `pattern.search(text, pos)` — leftmost match at or after `from`, with
    /// alternatives tried in source order (CPython, not a leftmost-longest DFA).
    pub fn search(&self, s: &[char], from: usize) -> Option<Mx> {
        let start = from.min(s.len());
        let mut caps = self.slots();
        let mut lp = vec![UNSET; self.n_loops.max(1)];
        for pos in start..=s.len() {
            for x in caps.iter_mut() {
                *x = UNSET;
            }
            caps[0] = pos;
            let mut budget: i64 = 60_000_000;
            if let Some(end) = self.m(s, &self.prog, 0, pos, &mut caps, &mut lp, &mut budget) {
                caps[1] = end;
                return Some(Mx { start: pos, end, caps });
            }
        }
        None
    }

    /// `pattern.search(text, pos)` restricted to one exact starting position.
    pub fn search_at(&self, s: &[char], pos: usize) -> Option<Mx> {
        if pos > s.len() {
            return None;
        }
        let mut caps = self.slots();
        caps[0] = pos;
        let mut lp = vec![UNSET; self.n_loops.max(1)];
        let mut budget: i64 = 60_000_000;
        match self.m(s, &self.prog, 0, pos, &mut caps, &mut lp, &mut budget) {
            Some(end) => {
                caps[1] = end;
                Some(Mx { start: pos, end, caps })
            }
            None => None,
        }
    }

    /// `pattern.match(text)`
    pub fn match_(&self, s: &[char]) -> Option<Mx> {
        self.search_at(s, 0)
    }

    pub fn is_match(&self, s: &[char]) -> bool {
        self.match_(s).is_some()
    }

    pub fn searched(&self, s: &[char]) -> bool {
        self.search(s, 0).is_some()
    }

    fn fwd(&self, pc: usize, off: isize) -> usize {
        ((pc as isize) + off).max(0) as usize
    }

    /// Does the single one-character instruction `it` match `s[pos]`?
    fn item_consumes(&self, it: &In, s: &[char], pos: usize) -> bool {
        let n = s.len();
        match it {
            In::Char(c) => pos < n && s[pos] == *c,
            In::CharI(c) => pos < n && eq_ic(s[pos], *c),
            In::Any => pos < n && s[pos] != '\n',
            In::AnyX => pos < n,
            In::Class(i) => pos < n && class_match(&self.classes[*i], s[pos]),
            _ => false,
        }
    }

    /// Recognise `SplitLoop -> <single 1-char item> -> JmpLoop(back here)`,
    /// i.e. repetition of one character class (`.*?`, `` `[^`]+` ``,
    /// `[\s\S]*`, `\s+`, ...), and return the loop's continuation pc.
    fn single_char_loop(
        &self,
        prog: &[In],
        pc: usize,
        body: usize,
        after: usize,
    ) -> Option<usize> {
        match prog.get(body)? {
            In::Char(_) | In::CharI(_) | In::Any | In::AnyX | In::Class(_) => {}
            _ => return None,
        }
        let jmp = body + 1;
        match prog.get(jmp)? {
            In::JmpLoop { head, after: ja, .. }
                if self.fwd(jmp, *head) == pc && self.fwd(jmp, *ja) == after =>
            {
                Some(after)
            }
            _ => None,
        }
    }

    /// One backtracking step of the matcher.
    ///
    /// This is an *iterative* virtual machine: instead of recursing in native
    /// stack frames it pushes [`MatchFrame`] records onto a heap `Vec`.  The
    /// distinction matters because a repetition whose body is not a single
    /// item (`(?:\\[\s\S]|[^\\$\n\r]|\n(?!\n))+`, the inline-math scanner of
    /// `texmd.py:910/1037`) consumed one *native* frame per character, which
    /// overflowed the guard page — an uncatchable `STATUS_STACK_OVERFLOW`
    /// abort — for an inline math span barely 100 characters long.  CPython's
    /// `sre` keeps its backtracking stack in a `SRE_STATE` block that grows on
    /// the heap, so it succeeds there; a heap continuation stack is what makes
    /// the two agree.  `budget` still counts one step per dispatched attempt
    /// exactly as before, so step-for-step behaviour (including the
    /// budget-exhausted `None`) is preserved.
    fn m<'p>(
        &self,
        s: &[char],
        prog: &'p [In],
        pc0: usize,
        pos0: usize,
        caps: &mut Vec<usize>,
        lp: &mut Vec<usize>,
        budget: &mut i64,
    ) -> Option<usize> {
        let mut frames: Vec<MatchCtx<'p>> = Vec::with_capacity(2);
        frames.push(MatchCtx {
            prog,
            caps: std::mem::take(caps),
            lp: std::mem::take(lp),
        });
        let mut stack: Vec<MatchFrame<'p>> = Vec::with_capacity(32);
        let mut cur = 0usize;
        let mut pc = pc0;
        let mut pos = pos0;
        let n = s.len();

        'run: loop {
            // ---- execute until this attempt yields Some(end) or fails ----
            let mut ret: Option<usize> = 'exec: loop {
                *budget -= 1;
                if *budget < 0 {
                    break 'exec None;
                }
                let p: &'p [In] = frames[cur].prog;
                if pc >= p.len() {
                    // implicit end of a look-around sub-program
                    break 'exec Some(pos);
                }
                match &p[pc] {
                    In::Match => break 'exec Some(pos),
                    In::Char(c) => {
                        let c = *c;
                        if pos < n && s[pos] == c {
                            pc += 1;
                            pos += 1;
                            continue 'exec;
                        }
                        break 'exec None;
                    }
                    In::CharI(c) => {
                        let c = *c;
                        if pos < n && eq_ic(s[pos], c) {
                            pc += 1;
                            pos += 1;
                            continue 'exec;
                        }
                        break 'exec None;
                    }
                    In::Any => {
                        if pos < n && s[pos] != '\n' {
                            pc += 1;
                            pos += 1;
                            continue 'exec;
                        }
                        break 'exec None;
                    }
                    In::AnyX => {
                        if pos < n {
                            pc += 1;
                            pos += 1;
                            continue 'exec;
                        }
                        break 'exec None;
                    }
                    In::Class(idx) => {
                        let idx = *idx;
                        if pos < n && class_match(&self.classes[idx], s[pos]) {
                            pc += 1;
                            pos += 1;
                            continue 'exec;
                        }
                        break 'exec None;
                    }
                    In::Start => {
                        if pos == 0 || s[pos - 1] == '\n' {
                            pc += 1;
                            continue 'exec;
                        }
                        break 'exec None;
                    }
                    In::StartStr => {
                        if pos == 0 {
                            pc += 1;
                            continue 'exec;
                        }
                        break 'exec None;
                    }
                    In::End => {
                        if pos == n || s[pos] == '\n' {
                            pc += 1;
                            continue 'exec;
                        }
                        break 'exec None;
                    }
                    In::EndStr => {
                        if pos == n {
                            pc += 1;
                            continue 'exec;
                        }
                        break 'exec None;
                    }
                    In::EndOptNl => {
                        if pos == n || (pos + 1 == n && s[pos] == '\n') {
                            pc += 1;
                            continue 'exec;
                        }
                        break 'exec None;
                    }
                    In::WordB => {
                        let a = pos > 0 && is_word(s[pos - 1]);
                        let b = pos < n && is_word(s[pos]);
                        if a != b {
                            pc += 1;
                            continue 'exec;
                        }
                        break 'exec None;
                    }
                    In::NotWordB => {
                        let a = pos > 0 && is_word(s[pos - 1]);
                        let b = pos < n && is_word(s[pos]);
                        if a == b {
                            pc += 1;
                            continue 'exec;
                        }
                        break 'exec None;
                    }
                    In::Save(k) => {
                        let k = *k;
                        let old = frames[cur].caps[k];
                        frames[cur].caps[k] = pos;
                        stack.push(MatchFrame::Save { ctx: cur, k, old });
                        pc += 1;
                        continue 'exec;
                    }
                    In::Split(a, b) => {
                        let (a, b) = (*a, *b);
                        stack.push(MatchFrame::Alt {
                            ctx: cur,
                            alt: self.fwd(pc, b),
                            pos,
                        });
                        pc = self.fwd(pc, a);
                        continue 'exec;
                    }
                    In::Jmp(t) => {
                        let t = *t;
                        pc = self.fwd(pc, t);
                        continue 'exec;
                    }
                    In::SplitLoop { body, after, slot, body_first } => {
                        let (body, after, slot, body_first) =
                            (*body, *after, *slot, *body_first);
                        let old = frames[cur].lp[slot];
                        frames[cur].lp[slot] = pos;
                        let b = self.fwd(pc, body);
                        let a = self.fwd(pc, after);
                        stack.push(MatchFrame::Loop { ctx: cur, slot, old });
                        // Depth-bounded fast path: `x*` / `x+` over a single
                        // character class.  The candidate end positions are a
                        // contiguous run, so trying them in order reproduces
                        // the generic recursion's exploration order exactly.
                        if let Some(cont) = self.single_char_loop(p, pc, b, a) {
                            let item = &p[b];
                            let mut ends: Vec<usize> = Vec::with_capacity(16);
                            let mut q = pos;
                            loop {
                                ends.push(q);
                                if !self.item_consumes(item, s, q) {
                                    break;
                                }
                                q += 1;
                            }
                            if body_first {
                                ends.reverse();
                            }
                            let first_end = ends[0];
                            stack.push(MatchFrame::Ends {
                                ctx: cur,
                                ends,
                                i: 1,
                                cont,
                            });
                            pc = cont;
                            pos = first_end;
                            continue 'exec;
                        }
                        let (first, alt) = if body_first { (b, a) } else { (a, b) };
                        stack.push(MatchFrame::Alt { ctx: cur, alt, pos });
                        pc = first;
                        continue 'exec;
                    }
                    In::JmpLoop { head, after, slot } => {
                        let (head, after, slot) = (*head, *after, *slot);
                        if frames[cur].lp[slot] == pos {
                            pc = self.fwd(pc, after);
                        } else {
                            pc = self.fwd(pc, head);
                        }
                        continue 'exec;
                    }
                    In::Backref(g) => {
                        let g = *g;
                        let (a, b) = match (
                            frames[cur].caps.get(2 * g),
                            frames[cur].caps.get(2 * g + 1),
                        ) {
                            (Some(x), Some(y)) if *x != UNSET && *y != UNSET => (*x, *y),
                            _ => break 'exec None,
                        };
                        let len = b - a;
                        if a > n || b > n || pos + len > n {
                            break 'exec None;
                        }
                        let ok = if self.flags.ignore_case {
                            (0..len).all(|i| eq_ic(s[a + i], s[pos + i]))
                        } else {
                            s[a..b] == s[pos..pos + len]
                        };
                        if ok {
                            pc += 1;
                            pos += len;
                            continue 'exec;
                        }
                        break 'exec None;
                    }
                    In::Look { neg, behind, prog: sub, width } => {
                        let (neg, behind) = (*neg, *behind);
                        let sub: &'p [In] = sub.as_slice();
                        let after = pc + 1;
                        let lpos = pos;
                        let widths: Vec<usize> = if behind {
                            match width {
                                Some(w) => vec![*w],
                                None => (1..=12).collect(),
                            }
                        } else {
                            Vec::new()
                        };
                        // First look-behind start that is not past position 0.
                        let mut wi = 0usize;
                        if behind {
                            while wi < widths.len() && widths[wi] > lpos {
                                wi += 1;
                            }
                            if wi >= widths.len() {
                                // no attempt possible at all: `hit == false`
                                if !neg {
                                    break 'exec None;
                                }
                                pc = after;
                                continue 'exec;
                            }
                        }
                        let st = if behind { lpos - widths[wi] } else { lpos };
                        stack.push(MatchFrame::Look {
                            ctx: cur,
                            neg,
                            behind,
                            pos: lpos,
                            after,
                            sub,
                            widths,
                            wi,
                        });
                        let mut sc = self.slots();
                        sc[0] = st;
                        let slp = vec![UNSET; self.n_loops.max(1)];
                        frames.push(MatchCtx { prog: sub, caps: sc, lp: slp });
                        cur = frames.len() - 1;
                        pc = 0;
                        pos = st;
                        continue 'exec;
                    }
                }
            };
            // ---- unwind: hand `ret` to the pending continuation ----
            loop {
                let target = stack.last().map_or(0, |f| f.ctx());
                while frames.len() - 1 > target {
                    frames.pop();
                }
                cur = target;
                let fr = match stack.pop() {
                    None => {
                        let root = &mut frames[0];
                        *caps = std::mem::take(&mut root.caps);
                        *lp = std::mem::take(&mut root.lp);
                        return ret;
                    }
                    Some(fr) => fr,
                };
                match fr {
                    MatchFrame::Save { k, old, .. } => {
                        if ret.is_none() {
                            frames[cur].caps[k] = old;
                        }
                        continue;
                    }
                    MatchFrame::Loop { slot, old, .. } => {
                        frames[cur].lp[slot] = old;
                        continue;
                    }
                    MatchFrame::Alt { alt, pos: apos, .. } => {
                        if ret.is_some() {
                            continue;
                        }
                        pc = alt;
                        pos = apos;
                        continue 'run;
                    }
                    MatchFrame::Ends { ends, i, cont, .. } => {
                        if ret.is_some() {
                            continue;
                        }
                        if i < ends.len() {
                            let next = ends[i];
                            stack.push(MatchFrame::Ends {
                                ctx: cur,
                                ends,
                                i: i + 1,
                                cont,
                            });
                            pc = cont;
                            pos = next;
                            continue 'run;
                        }
                        ret = None;
                        continue;
                    }
                    MatchFrame::Look { neg, behind, pos: lpos, after, sub, widths, wi, .. } => {
                        let hit = if behind {
                            matches!(ret, Some(e) if e == lpos)
                        } else {
                            ret.is_some()
                        };
                        if !hit && behind && wi + 1 < widths.len() {
                            let mut j = wi + 1;
                            while j < widths.len() && widths[j] > lpos {
                                j += 1;
                            }
                            if j < widths.len() {
                                let st = lpos - widths[j];
                                stack.push(MatchFrame::Look {
                                    ctx: cur,
                                    neg,
                                    behind,
                                    pos: lpos,
                                    after,
                                    sub,
                                    widths,
                                    wi: j,
                                });
                                let mut sc = self.slots();
                                sc[0] = st;
                                let slp = vec![UNSET; self.n_loops.max(1)];
                                frames.push(MatchCtx { prog: sub, caps: sc, lp: slp });
                                cur = frames.len() - 1;
                                pc = 0;
                                pos = st;
                                continue 'run;
                            }
                        }
                        if hit == !neg {
                            pc = after;
                            pos = lpos;
                            continue 'run;
                        }
                        ret = None;
                        continue;
                    }
                }
            }
        }
    }

    // ---- high level helpers ------------------------------------------------

    fn expand_tpl(&self, tpl: &[char], s: &[char], m: &Mx) -> Vec<char> {
        let mut out: Vec<char> = Vec::new();
        let mut i = 0usize;
        while i < tpl.len() {
            let c = tpl[i];
            if c != '\\' {
                out.push(c);
                i += 1;
                continue;
            }
            i += 1;
            if i >= tpl.len() {
                out.push('\\');
                break;
            }
            let d = tpl[i];
            i += 1;
            if d.is_ascii_digit() {
                let mut num = (d as u32 - '0' as u32) as usize;
                if i < tpl.len() {
                    let e = tpl[i];
                    if e.is_ascii_digit() {
                        let cand = num * 10 + (e as u32 - '0' as u32) as usize;
                        if cand <= self.n_groups {
                            num = cand;
                            i += 1;
                        }
                    }
                }
                if let Some(g) = m.grp(s, num) {
                    out.extend(g);
                }
                continue;
            }
            let lit = match d {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                'v' => '\u{0b}',
                'f' => '\u{0c}',
                'a' => '\u{07}',
                'b' => '\u{08}',
                x => x,
            };
            out.push(lit);
        }
        out
    }

    /// `re.sub(pat, tpl, text)` — replaces every non-overlapping match.
    pub fn replace_all(&self, s: &[char], tpl: &str) -> Vec<char> {
        let t: Vec<char> = tpl.chars().collect();
        self.replace_fn(s, &|m, buf| self.expand_tpl(&t, buf, m))
    }

    /// `re.sub(pat, callable, text)`
    pub fn replace_fn(
        &self,
        s: &[char],
        f: &dyn Fn(&Mx, &[char]) -> Vec<char>,
    ) -> Vec<char> {
        let n = s.len();
        let mut out: Vec<char> = Vec::new();
        let mut pos = 0usize;
        let mut last = 0usize;
        while pos <= n {
            let m = match self.search(s, pos) {
                Some(m) => m,
                None => break,
            };
            if m.start < last {
                break;
            }
            out.extend_from_slice(&s[last..m.start]);
            let rep = f(&m, s);
            if m.end == m.start {
                // CPython: a zero-width match is replaced, then the scanner
                // steps over one character (and cannot re-match there).
                out.extend(rep);
                if m.end < n {
                    out.push(s[m.end]);
                }
                pos = m.end + 1;
                last = pos;
            } else {
                out.extend(rep);
                pos = m.end;
                last = m.end;
            }
        }
        if last < n {
            out.extend_from_slice(&s[last..]);
        }
        out
    }

    /// `re.findall(pat, text)` with exactly one capture group -> that group.
    pub fn findall(&self, s: &[char]) -> Vec<String> {
        let mut out = Vec::new();
        for m in self.iter(s) {
            if self.n_groups == 0 {
                out.push(m.gs_or_empty(s, 0));
            } else {
                out.push(m.gs_or_empty(s, 1));
            }
        }
        out
    }

    /// `re.findall(pat, text)` returning every group of every match.
    pub fn findall_rows(&self, s: &[char]) -> Vec<Vec<Option<String>>> {
        let mut out = Vec::new();
        for m in self.iter(s) {
            let mut row = Vec::new();
            for g in 0..=self.n_groups {
                row.push(m.gs(s, g));
            }
            out.push(row);
        }
        out
    }

    pub fn iter<'b>(&'b self, s: &'b [char]) -> ReIter<'b> {
        ReIter { re: self, s, pos: 0, done: false }
    }

    /// `re.split(pat, text)` — capture groups are interleaved (`None` marks a
    /// group that did not participate, exactly like Python's `None`).
    pub fn split(&self, s: &[char]) -> Vec<Option<String>> {
        let n = s.len();
        let mut out: Vec<Option<String>> = Vec::new();
        let mut pos = 0usize;
        let mut last = 0usize;
        while pos <= n {
            match self.search(s, pos) {
                None => break,
                Some(m) => {
                    if m.start >= last {
                        out.push(Some(chars_to_string(&s[last..m.start])));
                        for g in 1..=self.n_groups {
                            out.push(m.gs(s, g));
                        }
                        last = m.end;
                    }
                    if m.end == m.start {
                        pos = m.end + 1;
                    } else {
                        pos = m.end;
                    }
                }
            }
        }
        out.push(Some(chars_to_string(&s[last..])));
        out
    }
}

pub struct ReIter<'a> {
    re: &'a Re,
    s: &'a [char],
    pos: usize,
    done: bool,
}

impl<'a> Iterator for ReIter<'a> {
    type Item = Mx;
    fn next(&mut self) -> Option<Mx> {
        if self.done || self.pos > self.s.len() {
            return None;
        }
        let m = self.re.search(self.s, self.pos)?;
        if m.end == m.start {
            self.pos = m.end + 1;
        } else {
            self.pos = m.end;
        }
        Some(m)
    }
}

// ===========================================================================
// 2. Balanced-Brace & Argument Scanner  (texmd.py:32-78)
// ===========================================================================

/// `extract_balanced(text, start_pos, open_char, close_char)`.
///
/// Returns `(content, index_after_close)`; `(None, start_pos)` when no opening
/// delimiter is found at (or after leading whitespace from) `start_pos`.
/// Escaped delimiters (`\{`, `\}`) are skipped.  When no matching close
/// delimiter exists the rest of the text is returned, exactly like Python.
pub fn extract_balanced(
    text: &[char],
    start_pos: usize,
    open_char: char,
    close_char: char,
) -> (Option<Vec<char>>, usize) {
    let len = text.len();
    let mut pos = start_pos.min(len);
    while pos < len && py_isspace(text[pos]) {
        pos += 1;
    }
    if pos >= len || text[pos] != open_char {
        return (None, start_pos);
    }
    // Python's counter may go negative when an escaped open delimiter is
    // skipped (`\{a}`), which changes where the scan stops, so this is signed.
    let mut depth: i64 = 0;
    let start_idx = pos + 1;
    let mut i = pos;
    while i < len {
        let ch = text[i];
        if ch == '\\' {
            i += 2;
            continue;
        }
        if ch == open_char {
            depth += 1;
        } else if ch == close_char {
            depth -= 1;
            if depth == 0 {
                return (Some(py_slice(text, start_idx, i)), i + 1);
            }
        }
        i += 1;
    }
    (Some(py_slice(text, start_idx, len)), len)
}

pub fn extract_opt_arg(text: &[char], start_pos: usize) -> (Option<Vec<char>>, usize) {
    extract_balanced(text, start_pos, '[', ']')
}

pub fn extract_mand_arg(text: &[char], start_pos: usize) -> (Option<Vec<char>>, usize) {
    extract_balanced(text, start_pos, '{', '}')
}

// ===========================================================================
// 3. Macro Pre-Expansion Engine  (texmd.py:85-251)
// ===========================================================================

#[derive(Clone, Debug, PartialEq)]
pub struct MacroDef {
    pub num_args: i64,
    pub body: String,
}

/// Insertion-ordered macro table.  Python's `dict` keeps the position of an
/// existing key when it is re-assigned, which changes expansion order, so the
/// port models that explicitly.
#[derive(Clone, Debug, Default)]
pub struct MacroTable {
    entries: Vec<(String, MacroDef)>,
}

impl MacroTable {
    pub fn new() -> MacroTable {
        MacroTable { entries: Vec::new() }
    }
    pub fn set(&mut self, name: &str, def: MacroDef) {
        for e in self.entries.iter_mut() {
            if e.0 == name {
                e.1 = def;
                return;
            }
        }
        self.entries.push((name.to_string(), def));
    }
    pub fn get(&self, name: &str) -> Option<&MacroDef> {
        self.entries.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn items(&self) -> &[(String, MacroDef)] {
        &self.entries
    }
}

pub struct MacroExpander {
    pub macros: MacroTable,
}

/// Number of expansion rounds in `MacroExpander.expand()`'s `max_depth`
/// default (texmd.py:199).
pub const MACRO_MAX_DEPTH: usize = 5;

impl Default for MacroExpander {
    fn default() -> Self {
        MacroExpander::new()
    }
}

impl MacroExpander {
    /// Built-in high-frequency math shorthands, in Python's literal order.
    pub fn new() -> MacroExpander {
        let mut me = MacroExpander { macros: MacroTable::new() };
        let builtins: [(&str, i64, &str); 10] = [
            ("R", 0, r"\mathbb{R}"),
            ("N", 0, r"\mathbb{N}"),
            ("Z", 0, r"\mathbb{Z}"),
            ("Q", 0, r"\mathbb{Q}"),
            ("C", 0, r"\mathbb{C}"),
            ("bs", 1, r"\mathbf{#1}"),
            ("degree", 0, r"^\circ"),
            ("tri", 0, r"\triangle"),
            ("i", 0, r"\mathrm{i}"),
            ("e", 0, r"\mathrm{e}"),
        ];
        for (n, a, b) in builtins.iter() {
            me.macros.set(n, MacroDef { num_args: *a, body: b.to_string() });
        }
        me
    }

    /// texmd.py:103-197 — pull `\newcommand`/`\renewcommand`, `\def` and
    /// `\DeclareMathOperator` definitions out of the preamble and return the
    /// text with those statements removed.
    pub fn parse_preamble_macros(&mut self, text: &str) -> String {
        let mut s = to_chars(text);

        // 1. \newcommand{\name}[num]{body} / \newcommand*{\name}[num]{body}
        {
            let cmd_pattern = Re::new(r"\\(?:re)?newcommand\*?\s*\{?\\([a-zA-Z]+)\}?", Fl::NONE);
            let mut pos = 0usize;
            let mut cleaned: Vec<char> = Vec::new();
            let mut last_pos = 0usize;
            loop {
                let m = match cmd_pattern.search(&s, pos) {
                    Some(m) => m,
                    None => {
                        cleaned.extend(py_slice(&s, last_pos, s.len()));
                        break;
                    }
                };
                let name = m.gs_or_empty(&s, 1);
                let mut p_end = m.end;
                let mut num_args: i64 = 0;
                let (opt_val, np) = extract_opt_arg(&s, p_end);
                p_end = np;
                if let Some(v) = opt_val {
                    let t = py_strip_chars(&v);
                    num_args = chars_to_string(&t).parse::<i64>().unwrap_or(0);
                }
                let (body_val, np) = extract_mand_arg(&s, p_end);
                p_end = np;
                match body_val {
                    Some(b) => {
                        self.macros
                            .set(&name, MacroDef { num_args, body: chars_to_string(&b) });
                        cleaned.extend(py_slice(&s, last_pos, m.start));
                        last_pos = p_end;
                        pos = p_end;
                    }
                    None => {
                        pos = p_end;
                    }
                }
            }
            s = cleaned;
        }

        // 2. \def\name#1#2{body}
        {
            let def_pattern = Re::new(r"\\def\s*\\([a-zA-Z]+)([\s#0-9]*)", Fl::NONE);
            let num_pat = Re::new(r"#[0-9]", Fl::NONE);
            let mut pos = 0usize;
            let mut cleaned: Vec<char> = Vec::new();
            let mut last_pos = 0usize;
            loop {
                let m = match def_pattern.search(&s, pos) {
                    Some(m) => m,
                    None => {
                        cleaned.extend(py_slice(&s, last_pos, s.len()));
                        break;
                    }
                };
                let name = m.gs_or_empty(&s, 1);
                let args_sig = m.gs_or_empty(&s, 2);
                let sig = to_chars(&args_sig);
                let num_args = num_pat.findall(&sig).len() as i64;
                let mut p_end = m.end;
                let (body_val, np) = extract_mand_arg(&s, p_end);
                p_end = np;
                match body_val {
                    Some(b) => {
                        self.macros
                            .set(&name, MacroDef { num_args, body: chars_to_string(&b) });
                        cleaned.extend(py_slice(&s, last_pos, m.start));
                        last_pos = p_end;
                        pos = p_end;
                    }
                    None => {
                        cleaned.extend(py_slice(&s, last_pos, p_end));
                        last_pos = p_end;
                        pos = p_end;
                    }
                }
            }
            s = cleaned;
        }

        // 3. \DeclareMathOperator{\name}{op}
        {
            let op_pattern =
                Re::new(r"\\DeclareMathOperator\*?\s*\{?\\([a-zA-Z]+)\}?", Fl::NONE);
            let mut pos = 0usize;
            let mut cleaned: Vec<char> = Vec::new();
            let mut last_pos = 0usize;
            loop {
                let m = match op_pattern.search(&s, pos) {
                    Some(m) => m,
                    None => {
                        cleaned.extend(py_slice(&s, last_pos, s.len()));
                        break;
                    }
                };
                let name = m.gs_or_empty(&s, 1);
                let mut p_end = m.end;
                let (body_val, np) = extract_mand_arg(&s, p_end);
                p_end = np;
                match body_val {
                    Some(b) => {
                        let body = format!("\\operatorname{{{}}}", chars_to_string(&b));
                        self.macros.set(&name, MacroDef { num_args: 0, body });
                        cleaned.extend(py_slice(&s, last_pos, m.start));
                        last_pos = p_end;
                        pos = p_end;
                    }
                    None => {
                        cleaned.extend(py_slice(&s, last_pos, p_end));
                        last_pos = p_end;
                        pos = p_end;
                    }
                }
            }
            s = cleaned;
        }

        chars_to_string(&s)
    }

    pub fn expand(&self, text: &str) -> String {
        chars_to_string(&self.expand_c(&to_chars(text), MACRO_MAX_DEPTH))
    }

    /// texmd.py:199-251.  `max_depth` is a *round* budget, not a nesting cap:
    /// each round sweeps every macro once and a macro's own replacement text is
    /// never re-scanned inside the same sweep.  When the budget runs out the
    /// remaining macro calls are simply left in the text — Python raises
    /// nothing and emits nothing special (measured: a self-referential
    /// `\newcommand{\L}{\L!}` expands to `\L!!!!!`, never an error).
    pub fn expand_c(&self, text: &[char], max_depth: usize) -> Vec<char> {
        if self.macros.is_empty() || max_depth == 0 {
            return text.to_vec();
        }
        let mut s = text.to_vec();
        for _ in 0..max_depth {
            let mut changed = false;
            for (name, meta) in self.macros.items().to_vec() {
                let pattern_src = format!("\\\\{}(?![a-zA-Z])", name);
                let pattern = Re::new(&pattern_src, Fl::NONE);
                let mut pos = 0usize;
                let mut out: Vec<char> = Vec::new();
                let mut last_pos = 0usize;
                loop {
                    let m = match pattern.search(&s, pos) {
                        Some(m) => m,
                        None => {
                            out.extend(py_slice(&s, last_pos, s.len()));
                            break;
                        }
                    };
                    out.extend(py_slice(&s, last_pos, m.start));
                    let mut cur_pos = m.end;
                    let num_args =
                        if meta.num_args < 0 { 0usize } else { meta.num_args as usize };
                    let mut args: Vec<Vec<char>> = Vec::new();
                    for _ in 0..num_args {
                        let (arg_val, np) = extract_mand_arg(&s, cur_pos);
                        cur_pos = np;
                        match arg_val {
                            Some(v) => args.push(v),
                            None => {
                                // fall back to a single non-whitespace character
                                while cur_pos < s.len() && py_isspace(s[cur_pos]) {
                                    cur_pos += 1;
                                }
                                if cur_pos < s.len() {
                                    args.push(vec![s[cur_pos]]);
                                    cur_pos += 1;
                                } else {
                                    args.push(Vec::new());
                                }
                            }
                        }
                    }
                    let mut body = to_chars(&meta.body);
                    for (i, arg_v) in args.iter().enumerate() {
                        let token = format!("#{}", i + 1);
                        body = py_replace_c(&body, &token, &chars_to_string(arg_v));
                    }
                    out.extend(body);
                    last_pos = cur_pos;
                    pos = cur_pos;
                    changed = true;
                }
                s = out;
            }
            if !changed {
                break;
            }
        }
        s
    }
}

// ===========================================================================
// 4. LaTeX -> Markdown high-precision parser  (texmd.py:258-1077)
// ===========================================================================

/// Pattern names are copied verbatim from `texmd.py`; templates are copied
/// verbatim from the Python *raw strings* because `Re::expand_tpl` decodes
/// `\\` and `\1` exactly like `re`'s template parser.
fn sub_c(s: &[char], pat: &str, tpl: &str, fl: Fl) -> Vec<char> {
    Re::new(pat, fl).replace_all(s, tpl)
}

fn sub_fn_c<F: Fn(&Mx, &[char]) -> Vec<char>>(
    s: &[char],
    pat: &str,
    fl: Fl,
    f: &F,
) -> Vec<char> {
    Re::new(pat, fl).replace_fn(s, f)
}

/// `plugin_runtime.latex_label()` — the optional `pylatexenc` plugin bridge.
/// See the module header: the Rust kernel has no plugin sandbox, so the
/// documented disabled-plugin behaviour (return the input unchanged) is ported.
pub fn latex_label(text: &str) -> String {
    text.to_string()
}

/// `os.path.join` for the two-argument form used by `_expand_inputs`.  Forward
/// slashes are used unconditionally: Win32 accepts them and it keeps the port
/// byte-identical to the Python sources that ship in this repo.
fn py_join(a: &str, b: &str) -> String {
    let a = a.trim_end_matches(['/', '\\']);
    if a.is_empty() {
        return b.to_string();
    }
    if b.starts_with('/') || b.starts_with('\\') || (b.len() > 1 && &b[1..2] == ":") {
        return b.to_string();
    }
    format!("{}/{}", a, b)
}

fn path_exists(p: &str) -> bool {
    !p.is_empty() && std::path::Path::new(p).exists()
}

fn path_is_file(p: &str) -> bool {
    !p.is_empty() && std::path::Path::new(p).is_file()
}

/// `os.path.dirname`
fn path_dirname(p: &str) -> String {
    match std::path::Path::new(p).parent() {
        Some(x) => x.to_string_lossy().to_string(),
        None => String::new(),
    }
}

/// `raw_b.decode('utf-8')` with the `latin-1` fallback from texmd.py:289-292.
fn decode_utf8_or_latin1(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|b| *b as char).collect(),
    }
}

/// texmd.py:273-303 `_expand_inputs` — recursion is guarded by the same
/// `depth > 10` cap as the Python original (11 is the highest level reached).
fn expand_inputs(t: &[char], cur_dir: &str, base_dir: &str, depth: usize) -> Vec<char> {
    if depth > 10 {
        return t.to_vec();
    }
    let pat = Re::new(r"\\(?:input|include|subfile)\{([^}]+)\}", Fl::NONE);
    pat.replace_fn(
        t,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let fname = py_strip_chars(&m.grp(buf, 1).unwrap_or_default());
            let fname = chars_to_string(&fname);
            let candidates: Vec<String> = vec![
                py_join(cur_dir, &fname),
                py_join(cur_dir, &(fname.clone() + ".tex")),
                py_join(base_dir, &fname),
                py_join(base_dir, &(fname.clone() + ".tex")),
            ];
            for target in candidates {
                if !path_is_file(&target) {
                    continue;
                }
                let raw = match std::fs::read(&target) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                let sub_content = decode_utf8_or_latin1(&raw);
                let sub_lines: Vec<String> = py_splitlines(&sub_content)
                    .into_iter()
                    .map(|l| {
                        chars_to_string(&sub_c(&to_chars(&l), r"(?<!\\)%.*$", "", Fl::NONE))
                    })
                    .collect();
                let mut sub_text = to_chars(&sub_lines.join("\n"));
                if contains_str(&sub_text, r"\begin{document}") {
                    let bt = to_chars(r"\begin{document}");
                    if let Some(i) = find_sub(&sub_text, &bt, 0) {
                        sub_text = sub_text[i + bt.len()..].to_vec();
                    }
                }
                if contains_str(&sub_text, r"\end{document}") {
                    let bt = to_chars(r"\end{document}");
                    if let Some(i) = find_sub(&sub_text, &bt, 0) {
                        sub_text = sub_text[..i].to_vec();
                    }
                }
                let inner =
                    expand_inputs(&sub_text, &path_dirname(&target), base_dir, depth + 1);
                let mut out: Vec<char> = Vec::new();
                out.extend(to_chars("\n\n").iter().cloned());
                out.extend(inner);
                out.extend(to_chars("\n\n").iter().cloned());
                return out;
            }
            Vec::new()
        },
    )
}

/// texmd.py:313-334 `_clean_metadata_text`
fn clean_metadata_text(val: &str) -> String {
    if val.is_empty() {
        return String::new();
    }
    let mut v = to_chars(val);
    v = sub_c(
        &v,
        r"\\(?:thanks|footnote|email|inst|affil|corref|fnmark|authornote)\{[^}]*\}",
        "",
        Fl::NONE,
    );
    v = sub_c(&v, r"\\footnotemark(?:\[[^\]]*\])?", "", Fl::NONE);
    v = sub_c(&v, r"\\(?:hspace|vspace)\*?\{[^}]*\}", "", Fl::NONE);
    v = sub_c(&v, r"\\color\{[^}]*\}", "", Fl::NONE);
    v = sub_c(
        &v,
        r"\\(?:texttt|textbf|textit|textsf|textsc|emph)\{([^}]*)\}",
        r"\1",
        Fl::NONE,
    );
    v = sub_c(&v, r"\\url\{([^}]*)\}", r"\1", Fl::NONE);
    v = sub_c(&v, r"\\href\{[^}]*\}\{([^}]*)\}", r"\1", Fl::NONE);
    v = sub_c(&v, r"\\(?:And|AND|and)\b", " & ", Fl::NONE);
    let mut v = latex_label(&chars_to_string(&v));
    v = py_replace(&v, "\\\\", " ");
    v = py_replace(&v, "\\", "");
    v = py_replace(&v, "{", "");
    v = py_replace(&v, "}", "");
    v = py_strip(&chars_to_string(&sub_c(&to_chars(&v), r"\s+", " ", Fl::NONE)));
    let mut vc = sub_c(
        &to_chars(&v),
        r"^(&\s*)+|(\s*&)+$",
        "",
        Fl::NONE,
    );
    vc = to_chars(&py_strip(&chars_to_string(&vc)));
    vc = py_replace_c(&vc, "\"", "'");
    chars_to_string(&vc)
}

#[derive(Clone, Copy)]
enum InlineKind {
    Wrap(&'static str, &'static str),
    Raw,
    Footnote,
}

/// texmd.py:937-955 `inline_map`, in Python's literal order.
fn inline_map() -> Vec<(&'static str, InlineKind)> {
    vec![
        (r"\\textbf\*?", InlineKind::Wrap("**", "**")),
        (r"\\textit\*?", InlineKind::Wrap("*", "*")),
        (r"\\emph\*?", InlineKind::Wrap("*", "*")),
        (r"\\textsl\*?", InlineKind::Wrap("*", "*")),
        (r"\\texttt\*?", InlineKind::Wrap("`", "`")),
        (r"\\path\*?", InlineKind::Wrap("`", "`")),
        (r"\\nolinkurl\*?", InlineKind::Wrap("`", "`")),
        (r"\\underline\*?", InlineKind::Wrap("<u>", "</u>")),
        (r"\\sout\*?", InlineKind::Wrap("~~", "~~")),
        (r"\\st\*?", InlineKind::Wrap("~~", "~~")),
        (
            r"\\textsc\*?",
            InlineKind::Wrap(
                "<span style=\"font-variant: small-caps;\">",
                "</span>",
            ),
        ),
        (r"\\textsubscript\*?", InlineKind::Wrap("<sub>", "</sub>")),
        (r"\\textsuperscript\*?", InlineKind::Wrap("<sup>", "</sup>")),
        (r"\\textsf\*?", InlineKind::Raw),
        (r"\\textmd\*?", InlineKind::Raw),
        (r"\\textup\*?", InlineKind::Raw),
        (r"\\footnote", InlineKind::Footnote),
    ]
}

/// `latex_to_md(tex_content, base_dir)` — texmd.py:258-1077.
pub fn latex_to_md(tex_content: &str, base_dir: &str) -> String {
    if tex_content.is_empty() || py_strip(tex_content).is_empty() {
        return String::new();
    }

    // 1. strip trailing comments (escaped \% is protected)
    let lines: Vec<String> = py_splitlines(tex_content)
        .into_iter()
        .map(|l| chars_to_string(&sub_c(&to_chars(&l), r"(?<!\\)%.*$", "", Fl::NONE)))
        .collect();
    let mut text = to_chars(&lines.join("\n"));

    // recursive \input / \include / \subfile expansion (only with a real dir)
    if !base_dir.is_empty() && path_exists(base_dir) {
        text = expand_inputs(&text, base_dir, base_dir, 0);
    }

    // 2. macro extraction + pre-expansion
    let mut macro_engine = MacroExpander::new();
    let cleaned = macro_engine.parse_preamble_macros(&chars_to_string(&text));
    text = macro_engine.expand_c(&to_chars(&cleaned), MACRO_MAX_DEPTH);

    // 3. document metadata
    let mut title_val = String::new();
    if let Some(m) = Re::new(r"\\title(?:\[[^\]]*\])?\{", Fl::NONE).search(&text, 0) {
        let (val, _) = extract_mand_arg(&text, m.end - 1);
        title_val = clean_metadata_text(&chars_to_string(&val.unwrap_or_default()));
    }
    let mut author_val = String::new();
    if let Some(m) = Re::new(r"\\author(?:\[[^\]]*\])?\{", Fl::NONE).search(&text, 0) {
        let (val, _) = extract_mand_arg(&text, m.end - 1);
        author_val = clean_metadata_text(&chars_to_string(&val.unwrap_or_default()));
    }
    let mut date_val = String::new();
    if let Some(m) = Re::new(r"\\date(?:\[[^\]]*\])?\{", Fl::NONE).search(&text, 0) {
        let (val, _) = extract_mand_arg(&text, m.end - 1);
        date_val = clean_metadata_text(&chars_to_string(&val.unwrap_or_default()));
    }

    // 4. body slice, keeping an abstract that sits in the preamble
    if contains_str(&text, r"\begin{document}") {
        let bt = to_chars(r"\begin{document}");
        let i = find_sub(&text, &bt, 0).unwrap();
        let doc_body = text[i + bt.len()..].to_vec();
        let preamble = text[..i].to_vec();
        let mut body = doc_body;
        let am = Re::new(r"\\begin\{abstract\}(.*?)\\end\{abstract\}", Fl::DOTALL)
            .search(&preamble, 0);
        if let Some(m) = am {
            let mut nb = m.grp(&preamble, 0).unwrap_or_default();
            nb.extend(to_chars("\n\n"));
            nb.extend(body);
            body = nb;
        }
        text = body;
    }
    if contains_str(&text, r"\end{document}") {
        let bt = to_chars(r"\end{document}");
        let i = find_sub(&text, &bt, 0).unwrap();
        text = text[..i].to_vec();
    }

    text = sub_c(&text, r"\\bgroup\b", "{", Fl::NONE);
    text = sub_c(&text, r"\\egroup\b", "}", Fl::NONE);
    text = sub_c(&text, r"\\textcolor\{[^}]*\}\{([^}]*)\}", r"\1", Fl::NONE);
    text = sub_c(&text, r"\\color\{[^}]*\}", "", Fl::NONE);

    // classical TeX quotes
    text = sub_c(&text, "``([^`\\n]*?)(''|\\\")", "“\\1”", Fl::NONE);
    text = sub_c(&text, "`([^`\\n]*?)('|\\\")", "‘\\1’", Fl::NONE);

    // \verb|...|
    text = sub_fn_c(
        &text,
        r"\\verb(.)(.*?)\1",
        Fl::NONE,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let mut out = vec!['`'];
            out.extend(m.grp(buf, 2).unwrap_or_default());
            out.push('`');
            out
        },
    );

    text = sub_c(&text, r"\\textbackslash(?![a-zA-Z])", "\\\\", Fl::NONE);
    text = sub_c(&text, r"\\textbar(?![a-zA-Z])", "|", Fl::NONE);
    text = sub_c(&text, r"\\textless(?![a-zA-Z])", "<", Fl::NONE);
    text = sub_c(&text, r"\\textgreater(?![a-zA-Z])", ">", Fl::NONE);
    text = sub_c(&text, r"\\textasciitilde(?![a-zA-Z])", "~", Fl::NONE);
    text = sub_c(&text, r"\\textasciicircum(?![a-zA-Z])", "^", Fl::NONE);
    text = sub_c(&text, r"\\(?:newline|linebreak)(?![a-zA-Z])", "\\n", Fl::NONE);
    text = sub_c(&text, r"\\centerline\{([^}]+)\}", r"\1", Fl::NONE);

    // journal metadata commands
    text = sub_c(&text, r"\\cormark(?:\[.*?\])?", "*", Fl::NONE);
    text = sub_c(&text, r"\\cortext(?:\[.*?\])?\{([^}]+)\}", "\\n\\n* \\1\\n\\n", Fl::NONE);
    text = sub_c(&text, r"\\printcredits(?![a-zA-Z])", "", Fl::NONE);
    text = sub_c(
        &text,
        r"\\bio(?:\{.*?\})?(.*?)\\endbio",
        "\\n\\n**作者简介：**\\n\\n\\1\\n\\n",
        Fl::DOTALL,
    );
    text = sub_c(&text, r"\\ead(?:\[.*?\])?\{([^}]+)\}", "<\\1>", Fl::NONE);

    // classic font switch environments
    text = sub_c(&text, r"\{\\bf\s+([^}]+)\}", "**\\1**", Fl::NONE);
    text = sub_c(&text, r"\{\\it\s+([^}]+)\}", "*\\1*", Fl::NONE);
    text = sub_c(&text, r"\{\\em\s+([^}]+)\}", "*\\1*", Fl::NONE);
    text = sub_c(&text, r"\{\\tt\s+([^}]+)\}", "`\\1`", Fl::NONE);
    text = sub_c(
        &text,
        r"\{\\sc\s+([^}]+)\}",
        "<span style=\"font-variant: small-caps;\">\\1</span>",
        Fl::NONE,
    );
    text = sub_c(&text, r"\{\\rm\s+([^}]+)\}", r"\1", Fl::NONE);
    text = sub_c(&text, r"\{\\sf\s+([^}]+)\}", r"\1", Fl::NONE);

    // bare font switches
    text = sub_c(&text, r"\\bf\s+([^\n\\{]+)", "**\\1**", Fl::NONE);
    text = sub_c(&text, r"\\it\s+([^\n\\{]+)", "*\\1*", Fl::NONE);
    text = sub_c(&text, r"\\tt\s+([^\n\\{]+)", "`\\1`", Fl::NONE);
    text = sub_c(&text, r"\\(?:rm|sf|em|sc)\s+([^\n\\{]+)", r"\1", Fl::NONE);

    // page structure
    text = sub_c(
        &text,
        r"\\(?:maketitle|tableofcontents|newpage|clearpage|cleardoublepage)",
        "",
        Fl::NONE,
    );
    text = sub_c(&text, r"\\(?:vspace\*?|hspace\*?)\{[^}]*\}", "", Fl::NONE);
    text = sub_c(
        &text,
        r"\\(?:vskip|hskip|kern)\s*[-+]?\d+(?:\.\d+)?[a-zA-Z]+",
        "",
        Fl::NONE,
    );
    text = sub_c(
        &text,
        r"\\(?:centering|raggedright|raggedleft|noindent|indent|vfill|hfill|smallskip|medskip|bigskip)(?![a-zA-Z])",
        "",
        Fl::NONE,
    );
    text = sub_c(
        &text,
        r"\\(?:Huge|huge|LARGE|Large|large|normalsize|small|footnotesize|scriptsize|tiny)(?![a-zA-Z])",
        "",
        Fl::NONE,
    );
    text = sub_c(&text, r"\\rule\{[^}]*\}\{[^}]*\}", "\\n\\n---\\n\\n", Fl::NONE);
    text = sub_c(&text, r"\\hrule(?![a-zA-Z])", "\\n\\n---\\n\\n", Fl::NONE);

    // 5. code listings
    text = sub_fn_c(
        &text,
        r"\\begin\{lstlisting\}(?:\[(.*?)\])?(.*?)\\end\{lstlisting\}",
        Fl::DOTALL,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let opt = m.grp(buf, 1).unwrap_or_default();
            let body = m.grp(buf, 2).unwrap_or_default();
            let mut lang = String::new();
            let lm = Re::new(r"language\s*=\s*([a-zA-Z0-9_\+#]+)", Fl::IC).search(&opt, 0);
            if let Some(x) = lm {
                lang = chars_to_string(&x.grp(&opt, 1).unwrap_or_default()).to_lowercase();
            }
            let mut out = to_chars(&format!("\n```{}\n", lang));
            out.extend(py_strip_chars(&body));
            out.extend(to_chars("\n```\n"));
            out
        },
    );
    text = sub_c(
        &text,
        r"\\begin\{minted\}(?:\[.*?\])?\{([a-zA-Z0-9_\+#]+)\}(.*?)\\end\{minted\}",
        "\\n```\\1\\n\\2\\n```\\n",
        Fl::DOTALL,
    );
    text = sub_c(
        &text,
        r"\\begin\{(?:verbatim\*?|stdout|session|shell|console|terminal|alltt|code)\}(.*?)\\end\{(?:verbatim\*?|stdout|session|shell|console|terminal|alltt|code)\}",
        "\\n```\\n\\1\\n```\\n",
        Fl::DOTALL,
    );
    text = sub_c(&text, r"\\verb([^a-zA-Z0-9\s])(.*?)\1", "`\\2`", Fl::NONE);

    // \resizebox / \scalebox / \parbox unwrapping
    text = unwrap_boxes(&text);

    // algorithm environments
    text = sub_fn_c(
        &text,
        r"\\begin\{(?:algorithm\*?|algorithm2e)\}(?:\[.*?\])?(.*?)\\end\{(?:algorithm\*?|algorithm2e)\}",
        Fl::DOTALL,
        &|m: &Mx, buf: &[char]| repl_algorithm(m, buf),
    );
    text = sub_fn_c(
        &text,
        r"\\begin\{algorithmic\*?\}(?:\[.*?\])?(.*?)\\end\{algorithmic\*?\}",
        Fl::DOTALL,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let mut out = to_chars("\n```pseudocode\n");
            out.extend(py_strip_chars(&m.grp(buf, 1).unwrap_or_default()));
            out.extend(to_chars("\n```\n"));
            out
        },
    );

    // 6. math delimiters
    text = sub_c(&text, r"\\\[(.*?)\\\]", "\\n\\n$$\\n\\1\\n$$\\n\\n", Fl::DOTALL);
    text = sub_c(&text, r"\\\s*\\\)", r"\)", Fl::NONE);
    text = sub_c(&text, r"\\\(\s*(.*?)\s*\\\)", "$\\1$", Fl::DOTALL);
    text = sub_c(&text, r"\\\(", "$", Fl::NONE);
    text = sub_c(&text, r"\\\)", "$", Fl::NONE);

    let math_block_envs: [&str; 14] = [
        "equation",
        "equation*",
        "align",
        "align*",
        "gather",
        "gather*",
        "multline",
        "multline*",
        "flalign",
        "flalign*",
        "alignat",
        "alignat*",
        "eqnarray",
        "eqnarray*",
    ];
    for env in math_block_envs.iter() {
        let escaped = re_escape(env);
        let pattern = format!(
            r"\\begin\{{{}}}(?:\[.*?\])?(.*?)\\end\{{{}}}",
            escaped, escaped
        );
        text = sub_fn_c(&text, &pattern, Fl::DOTALL, &|m: &Mx, buf: &[char]| -> Vec<char> {
            let mut body = py_strip_chars(&m.grp(buf, 1).unwrap_or_default());
            body = sub_c(&body, r"\\label\{[^}]*\}", "", Fl::NONE);
            let body = py_strip_chars(&body);
            let head = to_chars("\n\n$$\n");
            let mut out: Vec<char> = head;
            if *env == "equation" || *env == "equation*" {
                out.extend(body.iter().cloned());
                out.extend(to_chars("\n$$\n\n"));
                return out;
            }
            if *env == "eqnarray" || *env == "eqnarray*" {
                out.extend(to_chars("\\begin{aligned}\n"));
                out.extend(body.iter().cloned());
                out.extend(to_chars("\n\\end{aligned}\n$$\n\n"));
                return out;
            }
            out.extend(to_chars(&format!("\\begin{{{}}}\n", env)));
            out.extend(body.iter().cloned());
            out.extend(to_chars(&format!("\n\\end{{{}}}\n$$\n\n", env)));
            out
        });
    }

    // 7. lists
    text = sub_fn_c(
        &text,
        r"\\begin\{itemize\}(.*?)\\end\{itemize\}",
        Fl::DOTALL,
        &|m: &Mx, buf: &[char]| repl_itemize(m, buf, false),
    );
    text = sub_fn_c(
        &text,
        r"\\begin\{enumerate\}(.*?)\\end\{enumerate\}",
        Fl::DOTALL,
        &|m: &Mx, buf: &[char]| repl_itemize(m, buf, true),
    );
    text = sub_fn_c(
        &text,
        r"\\begin\{description\}(.*?)\\end\{description\}",
        Fl::DOTALL,
        &|m: &Mx, buf: &[char]| repl_itemize(m, buf, false),
    );
    text = sub_fn_c(
        &text,
        r"\\begin\{circlelist\*?\}(.*?)\\end\{circlelist\*?\}",
        Fl::DOTALL,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let body = py_strip_chars(&m.grp(buf, 1).unwrap_or_default());
            let items = Re::new(r"\\item(?:\s+|(?=[\\$]))", Fl::NONE).split(&body);
            let circle_nums = ['①', '②', '③', '④', '⑤', '⑥', '⑦', '⑧', '⑨', '⑩'];
            let mut res: Vec<String> = Vec::new();
            let mut idx = 0usize;
            for it in items.iter() {
                let raw = it.clone().unwrap_or_default();
                let it2 = py_strip(&raw);
                if it2.is_empty() {
                    continue;
                }
                let c_num = if idx < circle_nums.len() {
                    circle_nums[idx].to_string()
                } else {
                    format!("({})", idx + 1)
                };
                res.push(format!("- **{}** {}", c_num, it2));
                idx += 1;
            }
            to_chars(&format!("\n\n{}\n\n", res.join("\n")))
        },
    );
    // orphan \item fallback
    text = sub_fn_c(
        &text,
        r"\\item(?:\[(.*?)\])?(?:\s+|(?=[\\$]))",
        Fl::NONE,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            match m.grp(buf, 1) {
                Some(g) if !py_strip_chars(&g).is_empty() && !g.is_empty() => {
                    let tag = chars_to_string(&g);
                    to_chars(&format!("- **{}** ", tag))
                }
                _ => to_chars("- "),
            }
        },
    );

    // 8. theorem-like environments
    let theorem_map: [(&str, &str); 9] = [
        ("theorem", "定理 (Theorem)"),
        ("lemma", "引理 (Lemma)"),
        ("definition", "定义 (Definition)"),
        ("proposition", "命题 (Proposition)"),
        ("corollary", "推论 (Corollary)"),
        ("conjecture", "猜想 (Conjecture)"),
        ("proof", "证明 (Proof)"),
        ("remark", "注记 (Remark)"),
        ("example", "示例 (Example)"),
    ];
    for (thm_env, thm_title) in theorem_map.iter() {
        let pattern = format!(
            r"\\begin\{{{}}}(?:\[(.*?)\])?(.*?)\\end\{{{}}}",
            thm_env, thm_env
        );
        let title = *thm_title;
        text = sub_fn_c(&text, &pattern, Fl::DOTALL_IC, &|m: &Mx, buf: &[char]| -> Vec<char> {
            let opt_title = m.grp(buf, 1);
            let th_body = py_strip_chars(&m.grp(buf, 2).unwrap_or_default());
            let header = match &opt_title {
                Some(v) if !v.is_empty() => {
                    format!("**{} ({})**", title, py_strip(&chars_to_string(v)))
                }
                _ => format!("**{}**", title),
            };
            let quoted: Vec<String> = py_splitlines(&chars_to_string(&th_body))
                .into_iter()
                .map(|l| format!("> {}", l))
                .collect();
            to_chars(&format!("\n\n> {}\n>\n{}\n\n", header, quoted.join("\n")))
        });
    }

    // 9. exam / problem-set environments
    text = repl_choices(&text);
    text = sub_fn_c(
        &text,
        r"\\begin\{problem\*?\}(?:\[.*?\])?(.*?)\\end\{problem\*?\}",
        Fl::DOTALL,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let body = py_strip_chars(&m.grp(buf, 1).unwrap_or_default());
            to_chars(&format!("\n\n#### 【题目】\n\n{}\n\n", chars_to_string(&body)))
        },
    );
    text = sub_fn_c(
        &text,
        r"\\begin\{answer\*?\}(?:\[.*?\])?(.*?)\\end\{answer\*?\}",
        Fl::DOTALL,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let body = py_strip_chars(&m.grp(buf, 1).unwrap_or_default());
            to_chars(&format!("\n\n> **【答案】** {}\n\n", chars_to_string(&body)))
        },
    );
    text = sub_fn_c(
        &text,
        r"\\begin\{solution\*?\}(?:\[.*?\])?(.*?)\\end\{solution\*?\}",
        Fl::DOTALL,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let body = py_strip_chars(&m.grp(buf, 1).unwrap_or_default());
            let quoted: Vec<String> = py_splitlines(&chars_to_string(&body))
                .into_iter()
                .map(|l| format!("> {}", l))
                .collect();
            to_chars(&format!(
                "\n\n> **【解析】**\n>\n{}\n\n",
                quoted.join("\n")
            ))
        },
    );

    // 10. centering / quoting blocks
    text = sub_c(
        &text,
        r"\\begin\{(?:center|flushleft|flushright)\}(.*?)\\end\{(?:center|flushleft|flushright)\}",
        "\\n\\n\\1\\n\\n",
        Fl::DOTALL,
    );
    text = sub_fn_c(
        &text,
        r"\\begin\{(?:quote|quotation)\}(.*?)\\end\{(?:quote|quotation)\}",
        Fl::DOTALL,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let body = py_strip_chars(&m.grp(buf, 1).unwrap_or_default());
            let quoted: Vec<String> = py_splitlines(&chars_to_string(&body))
                .into_iter()
                .map(|l| format!("> {}", l))
                .collect();
            to_chars(&format!("\n\n{}\n\n", quoted.join("\n> ")))
        },
    );

    // 11. figures and fill-in-the-blank markers
    text = sub_c(&text, r"\\fillinblank(?:\{[^}]*\})?", " ______ ", Fl::NONE);
    text = sub_c(&text, r"\\blank(?:\{[^}]*\})?", " ______ ", Fl::NONE);
    text = sub_c(
        &text,
        r"\\solutionfigure\{\\bitmapfigure(?:\[.*?\])?\{([^}]+)\}\}",
        "\\n\\n![解析配图](\\1)\\n\\n",
        Fl::NONE,
    );
    text = sub_c(
        &text,
        r"\\bitmapfigure(?:\[.*?\])?\{([^}]+)\}",
        "\\n\\n![题目配图](\\1)\\n\\n",
        Fl::NONE,
    );
    text = sub_c(
        &text,
        r"\\Figure(?:Layout|Trim)Declare\{[^}]*\}\{[^}]*\}\{[^}]*\}",
        "",
        Fl::NONE,
    );
    text = sub_c(
        &text,
        r"\\Figure(?:Layout|Trim)Declare\{[^}]*\}\{[^}]*\}",
        "",
        Fl::NONE,
    );

    text = sub_fn_c(
        &text,
        r"\\begin\{subfigure\}(?:\[.*?\])?(?:\{.*?\})?(.*?)\\end\{subfigure\}",
        Fl::DOTALL,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let sf_body = m.grp(buf, 1).unwrap_or_default();
            let caption = extract_caption(&sf_body);
            let img_m =
                Re::new(r"\\includegraphics(?:\[.*?\])?\{([^}]+)\}", Fl::NONE).search(&sf_body, 0);
            let img_path = match img_m {
                Some(x) => py_strip(&chars_to_string(&x.grp(&sf_body, 1).unwrap_or_default())),
                None => String::new(),
            };
            if !img_path.is_empty() {
                return to_chars(&format!("\n![{}]({})\n", caption, img_path));
            }
            Vec::new()
        },
    );

    text = sub_fn_c(
        &text,
        r"\\begin\{(?:figure\*?|wrapfigure|sidewaysfigure)\}(?:\[.*?\])?(.*?)\\end\{(?:figure\*?|wrapfigure|sidewaysfigure)\}",
        Fl::DOTALL,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let f_body = m.grp(buf, 1).unwrap_or_default();
            let caption = extract_caption(&f_body);
            let imgs: Vec<String> =
                Re::new(r"\\includegraphics(?:\[.*?\])?\{([^}]+)\}", Fl::NONE)
                    .findall(&f_body)
                    .into_iter()
                    .collect();
            if !imgs.is_empty() {
                let res: Vec<String> = imgs
                    .iter()
                    .map(|p| format!("![{}]({})", caption, py_strip(p)))
                    .collect();
                return to_chars(&format!("\n\n{}\n\n", res.join("\n\n")));
            }
            if !caption.is_empty() {
                return to_chars(&format!("\n\n**图：{}**\n\n", caption));
            }
            Vec::new()
        },
    );

    // 12. tables
    text = sub_fn_c(
        &text,
        r"\\begin\{(?:table\*?|wraptable|sidewaystable)\}(?:\[.*?\])?(.*?)\\end\{(?:table\*?|wraptable|sidewaystable)\}",
        Fl::DOTALL,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let tbl_body = m.grp(buf, 1).unwrap_or_default();
            let caption = extract_caption(&tbl_body);
            let caption_text = if caption.is_empty() {
                String::new()
            } else {
                format!("\n\n**表：{}**\n", caption)
            };
            let mut tab_parsed = parse_tabular_blocks(&tbl_body);
            tab_parsed = strip_caption(&tab_parsed);
            tab_parsed = sub_c(&tab_parsed, r"\\label\{[^}]*\}", "", Fl::NONE);
            tab_parsed = sub_c(
                &tab_parsed,
                r"\\(?:centering|raggedright|raggedleft)\b",
                "",
                Fl::NONE,
            );
            tab_parsed = sub_c(&tab_parsed, r"\\begin\{center\}|\\end\{center\}", "", Fl::NONE);
            let tab_parsed = py_strip_chars(&tab_parsed);
            let mut out = Vec::new();
            if !tab_parsed.is_empty() {
                out.extend(to_chars(&caption_text));
                out.extend(tab_parsed);
                return out;
            }
            out.extend(to_chars(&caption_text));
            out
        },
    );
    text = parse_tabular_blocks(&text);

    // abstract / keywords / acknowledgements / appendix / biography
    text = sub_fn_c(
        &text,
        r"\\begin\{abstract\}(.*?)\\end\{abstract\}",
        Fl::DOTALL,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let body = py_strip_chars(&m.grp(buf, 1).unwrap_or_default());
            let quoted: Vec<String> = py_splitlines(&chars_to_string(&body))
                .into_iter()
                .map(|l| format!("> {}", l))
                .collect();
            to_chars(&format!("\n\n> **摘要 (Abstract)**\n>\n{}\n\n", quoted.join("\n")))
        },
    );
    let kw_repl = |m: &Mx, buf: &[char]| -> Vec<char> {
        let body = py_strip_chars(&m.grp(buf, 1).unwrap_or_default());
        to_chars(&format!(
            "\n\n**关键词 (Keywords):** {}\n\n",
            chars_to_string(&body)
        ))
    };
    text = sub_fn_c(
        &text,
        r"\\begin\{(?:IEEEkeywords|keywords|keywords\*)\}(?:\[.*?\])?(.*?)\\end\{(?:IEEEkeywords|keywords|keywords\*)\}",
        Fl::DOTALL,
        &kw_repl,
    );
    text = sub_c(
        &text,
        r"\\keywords\{([^}]+)\}",
        "\\n\\n**关键词 (Keywords):** \\1\\n\\n",
        Fl::NONE,
    );
    text = sub_c(
        &text,
        r"\\begin\{(?:acknowledgements|acknowledgments|acks|acks\*)\}(.*?)\\end\{(?:acknowledgements|acknowledgments|acks|acks\*)\}",
        "\\n\\n## 致谢 (Acknowledgements)\\n\\n\\1\\n\\n",
        Fl::DOTALL,
    );
    text = sub_c(
        &text,
        r"\\section\*?\{(?:Acknowledgements|Acknowledgments|Acks)\}",
        "## 致谢 (Acknowledgements)",
        Fl::IC,
    );
    text = sub_c(
        &text,
        r"\\begin\{appendix\}(.*?)\\end\{appendix\}",
        "\\n\\n# 附录 (Appendix)\\n\\n\\1\\n\\n",
        Fl::DOTALL,
    );
    text = sub_c(&text, r"\\appendix(?![a-zA-Z])", "\\n\\n# 附录 (Appendix)\\n\\n", Fl::NONE);
    text = sub_c(
        &text,
        r"\\begin\{(?:IEEEbiography|IEEEbiographynophoto)\}(?:\[.*?\])?(?:\{.*?\})?(.*?)\\end\{(?:IEEEbiography|IEEEbiographynophoto)\}",
        "\\n\\n**作者简介：**\\n\\n\\1\\n\\n",
        Fl::DOTALL,
    );

    // section hierarchies
    let sec_commands: [(&str, &str); 7] = [
        (r"\\part\*?", "# "),
        (r"\\chapter\*?", "# "),
        (r"\\section\*?", "# "),
        (r"\\subsection\*?", "## "),
        (r"\\subsubsection\*?", "### "),
        (r"\\paragraph\*?", "#### "),
        (r"\\subparagraph\*?", "##### "),
    ];
    for (cmd_regex, md_prefix) in sec_commands.iter() {
        text = rewrite_sec(&text, cmd_regex, md_prefix);
    }

    // 13. cross references and citations
    text = sub_c(&text, r"\\eqref\{([^}]+)\}", "(\\1)", Fl::NONE);
    text = sub_c(
        &text,
        r"\\(?:autoref|cref|Cref|nameref)\{([^}]+)\}",
        "[§\\1]",
        Fl::NONE,
    );
    text = sub_c(&text, r"\\ref\{([^}]+)\}", "[\\1]", Fl::NONE);
    text = sub_c(&text, r"\\pageref\{([^}]+)\}", "[p.\\1]", Fl::NONE);
    text = sub_fn_c(
        &text,
        r"\\(?:cite|citep|citet|parencite|textcite|citeauthor|citeyear|nocite|citealp|citealt)(?:\[.*?\])*\{([^}]+)\}",
        Fl::NONE,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let raw = m.grp(buf, 1).unwrap_or_default();
            let keys: Vec<String> = py_split_c(&raw, ',')
                .into_iter()
                .map(|k| py_strip(&chars_to_string(&k)))
                .filter(|k| !k.is_empty())
                .collect();
            let joined: Vec<String> = keys.iter().map(|k| format!("@{}", k)).collect();
            to_chars(&format!("[{}]", joined.join("; ")))
        },
    );

    // 14. protected inline formatting
    let math_pat = Re::new(
        r"(```.*?```|\$\$.*?\$\$|(?<!\\)\$(?:(?:\\[\s\S])|[^\\$\n\r]|\n(?!\n))+(?<!\\)\$)",
        Fl::DOTALL,
    );
    let mut math_placeholders: Vec<String> = Vec::new();
    {
        let parts = math_pat.split(&text);
        let mut out: Vec<char> = Vec::new();
        for (i, p) in parts.iter().enumerate() {
            let pc = to_chars(&p.clone().unwrap_or_default());
            if i % 2 == 1 {
                let cleaned = sub_c(&pc, r"\\textsc\{([^}]+)\}", r"\\mathrm{\1}", Fl::NONE);
                let pid = math_placeholders.len();
                math_placeholders.push(chars_to_string(&cleaned));
                out.extend(to_chars(&format!("§§MATH_{}§§", pid)));
            } else {
                out.extend(pc);
            }
        }
        text = out;
    }

    let footnotes: std::cell::RefCell<Vec<(usize, String)>> =
        std::cell::RefCell::new(Vec::new());

    text = sub_c(&text, r"\\xspace(?![a-zA-Z])", "", Fl::NONE);

    for _ in 0..3 {
        let mut changed = false;
        for (cmd_tag, kind) in inline_map() {
            let pattern_src = format!("{}(?![a-zA-Z])", cmd_tag);
            let pattern = Re::new(&pattern_src, Fl::NONE);
            let mut pos = 0usize;
            let mut out: Vec<char> = Vec::new();
            let mut last_pos = 0usize;
            loop {
                let m = match pattern.search(&text, pos) {
                    Some(m) => m,
                    None => {
                        out.extend(py_slice(&text, last_pos, text.len()));
                        break;
                    }
                };
                out.extend(py_slice(&text, last_pos, m.start));
                let p_end = m.end;
                let (arg_val, p_end) = extract_mand_arg(&text, p_end);
                match arg_val {
                    Some(v) => {
                        let rep = match kind {
                            InlineKind::Wrap(a, b) => {
                                format!("{}{}{}", a, chars_to_string(&v), b)
                            }
                            InlineKind::Raw => chars_to_string(&v),
                            InlineKind::Footnote => {
                                let content = py_strip(&chars_to_string(&v));
                                let mut f = footnotes.borrow_mut();
                                let fn_id = f.len() + 1;
                                f.push((fn_id, content));
                                format!("[^{}]", fn_id)
                            }
                        };
                        out.extend(to_chars(&rep));
                        last_pos = p_end;
                        pos = p_end;
                        changed = true;
                    }
                    None => {
                        out.extend(py_slice(&text, m.start, p_end));
                        last_pos = p_end;
                        pos = p_end;
                    }
                }
            }
            text = out;
        }
        if !changed {
            break;
        }
    }

    for (pid, p) in math_placeholders.iter().enumerate() {
        let token = format!("§§MATH_{}§§", pid);
        text = py_replace_c(&text, &token, p);
    }

    text = repl_href(&text);
    text = sub_c(&text, r"\\url\{([^}]+)\}", "<\\1>", Fl::NONE);

    // 15. bibliography
    text = sub_fn_c(
        &text,
        r"\\begin\{thebibliography\}(?:\{[^}]*\})?(.*?)\\end\{thebibliography\}",
        Fl::DOTALL,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let body = py_strip_chars(&m.grp(buf, 1).unwrap_or_default());
            let items =
                Re::new(r"\\bibitem(?:\[(.*?)\])?\{([^}]+)\}", Fl::NONE).split(&body);
            let mut bib_lines: Vec<String> =
                vec!["\n\n## 参考文献 (References)\n".to_string()];
            let mut i = 1usize;
            while i < items.len() {
                let label = items[i].clone();
                let key = items
                    .get(i + 1)
                    .and_then(|x| x.clone())
                    .unwrap_or_default();
                let desc = match items.get(i + 2) {
                    Some(Some(v)) => py_strip(v),
                    _ => String::new(),
                };
                let prefix = match &label {
                    Some(l) if !l.is_empty() => format!("**[{}]**", l),
                    _ => format!("**[@{}]**", key),
                };
                bib_lines.push(format!("- {} {}", prefix, desc));
                i += 3;
            }
            to_chars(&format!("{}\n\n", bib_lines.join("\n")))
        },
    );

    // 16. plain-text unescaping outside code/math spans
    {
        let pattern = Re::new(
            r"(```.*?```|`[^`\n]+`|\$\$.*?\$\$|(?<!\\)\$(?:(?:\\[\s\S])|[^\\$\n\r]|\n(?!\n))+(?<!\\)\$)",
            Fl::DOTALL,
        );
        let parts = pattern.split(&text);
        let mut out: Vec<char> = Vec::new();
        for (i, p) in parts.iter().enumerate() {
            let mut pc = to_chars(&p.clone().unwrap_or_default());
            if i % 2 == 1 {
                out.extend(pc);
                continue;
            }
            pc = sub_c(&pc, "``(.*?)''", "“\\1”", Fl::DOTALL);
            pc = sub_c(&pc, "`(.*?)'", "‘\\1’", Fl::DOTALL);
            for (a, b) in [
                (r"\%", "%"),
                (r"\&", "&"),
                (r"\_", "_"),
                (r"\#", "#"),
                (r"\{", "{"),
                (r"\}", "}"),
                ("~", " "),
            ] {
                pc = py_replace_c(&pc, a, b);
            }
            pc = sub_c(
                &pc,
                r"(^|[\s\(\[{<])\$(?=[\s\)\]}>,.:;!?]|$)",
                "\\1\\\\$",
                Fl::NONE,
            );
            out.extend(pc);
        }
        text = out;
    }

    // 17. collected footnotes
    {
        let fns = footnotes.borrow();
        if !fns.is_empty() {
            let mut fn_lines: Vec<String> =
                vec!["\n\n---\n\n### 脚注 (Footnotes)\n".to_string()];
            for (fn_id, fn_text) in fns.iter() {
                fn_lines.push(format!("[^{}]: {}", fn_id, fn_text));
            }
            let mut t = text;
            t.push('\n');
            t.extend(to_chars(&fn_lines.join("\n")));
            t.push('\n');
            text = t;
        }
    }

    // 18. frontmatter
    let mut frontmatter: Vec<String> = Vec::new();
    if !title_val.is_empty() {
        frontmatter.push(format!("title: \"{}\"", title_val));
    }
    if !author_val.is_empty() {
        frontmatter.push(format!("author: \"{}\"", author_val));
    }
    if !date_val.is_empty() {
        frontmatter.push(format!("date: \"{}\"", date_val));
    }

    let cleaned_body = py_strip_chars(&sub_c(&text, r"\n{3,}", "\\n\\n", Fl::NONE));
    let mut res_parts: Vec<String> = Vec::new();
    if !frontmatter.is_empty() {
        res_parts.push(format!("---\n{}\n---\n", frontmatter.join("\n")));
    }
    res_parts.push(chars_to_string(&cleaned_body));
    py_strip(&res_parts.join("\n\n"))
}

/// texmd.py:376-381 `_repl_verb` is inlined above; this is texmd.py:440-462.
fn unwrap_boxes(t_in: &[char]) -> Vec<char> {
    let mut cur = t_in.to_vec();
    for cmd in [r"\\resizebox\*?", r"\\scalebox\*?", r"\\parbox\*?"].iter() {
        let pattern = Re::new(&format!("{}(?![a-zA-Z])", cmd), Fl::NONE);
        loop {
            let m = match pattern.search(&cur, 0) {
                Some(m) => m,
                None => break,
            };
            let mut p = m.end;
            if cmd.contains("resizebox") {
                let (_, np) = extract_mand_arg(&cur, p);
                p = np;
                let (_, np) = extract_mand_arg(&cur, p);
                p = np;
            } else if cmd.contains("parbox") {
                let (_, np) = extract_opt_arg(&cur, p);
                p = np;
                let (_, np) = extract_mand_arg(&cur, p);
                p = np;
            } else {
                let (_, np) = extract_mand_arg(&cur, p);
                p = np;
            }
            let (content, p_end) = extract_mand_arg(&cur, p);
            match content {
                Some(content) => {
                    let mut next: Vec<char> = Vec::new();
                    next.extend(py_slice(&cur, 0, m.start));
                    next.push(' ');
                    next.extend(content);
                    next.push(' ');
                    if p_end < cur.len() {
                        next.extend_from_slice(&cur[p_end..]);
                    }
                    cur = next;
                }
                None => break,
            }
        }
    }
    cur
}

/// texmd.py:467-493 `_repl_algorithm`
fn repl_algorithm(m: &Mx, buf: &[char]) -> Vec<char> {
    let algo_body = m.grp(buf, 1).unwrap_or_default();
    let cap_m = Re::new(r"\\caption\{([^}]+)\}", Fl::NONE).search(&algo_body, 0);
    let title = match &cap_m {
        Some(x) => py_strip(&chars_to_string(&x.grp(&algo_body, 1).unwrap_or_default())),
        None => "Algorithm".to_string(),
    };
    let mut code_lines: Vec<String> = Vec::new();
    for line in py_splitlines(&chars_to_string(&algo_body)) {
        let mut line = py_strip(&line);
        if line.is_empty() || line.starts_with(r"\caption") || line.starts_with(r"\label") {
            continue;
        }
        let lc = to_chars(&line);
        if Re::new(
            r"\\(?:begin|end)\{(?:algorithm\*?|algorithm2e|algorithmic\*?)\}",
            Fl::NONE,
        )
        .searched(&lc)
        {
            continue;
        }
        line = chars_to_string(&sub_c(&lc, r"\\(?:REQUIRE|INPUT)\b\s*", "**Input:** ", Fl::NONE));
        line = chars_to_string(&sub_c(
            &to_chars(&line),
            r"\\(?:ENSURE|OUTPUT)\b\s*",
            "**Output:** ",
            Fl::NONE,
        ));
        line = chars_to_string(&sub_c(&to_chars(&line), r"\\STATE\b\s*", "  ", Fl::NONE));
        line = chars_to_string(&sub_c(
            &to_chars(&line),
            r"\\FOR\{([^}]+)\}",
            "**for** \\1 **do**",
            Fl::NONE,
        ));
        line = chars_to_string(&sub_c(&to_chars(&line), r"\\ENDFOR\b", "**end for**", Fl::NONE));
        line = chars_to_string(&sub_c(
            &to_chars(&line),
            r"\\IF\{([^}]+)\}",
            "**if** \\1 **then**",
            Fl::NONE,
        ));
        line = chars_to_string(&sub_c(&to_chars(&line), r"\\ELSE\b", "**else**", Fl::NONE));
        line = chars_to_string(&sub_c(
            &to_chars(&line),
            r"\\ELSIF\{([^}]+)\}",
            "**else if** \\1 **then**",
            Fl::NONE,
        ));
        line = chars_to_string(&sub_c(&to_chars(&line), r"\\ENDIF\b", "**end if**", Fl::NONE));
        line = chars_to_string(&sub_c(
            &to_chars(&line),
            r"\\WHILE\{([^}]+)\}",
            "**while** \\1 **do**",
            Fl::NONE,
        ));
        line = chars_to_string(&sub_c(
            &to_chars(&line),
            r"\\ENDWHILE\b",
            "**end while**",
            Fl::NONE,
        ));
        line = chars_to_string(&sub_c(&to_chars(&line), r"\\RETURN\b\s*", "**return** ", Fl::NONE));
        code_lines.push(line);
    }
    to_chars(&format!(
        "\n\n**算法：{}**\n\n```pseudocode\n{}\n```\n\n",
        title,
        code_lines.join("\n")
    ))
}

/// texmd.py:529-562 — `_repl_itemize` / `_repl_enumerate` share the same split
/// logic; `ordered` selects the `1.` numbering.
fn repl_itemize(m: &Mx, buf: &[char], ordered: bool) -> Vec<char> {
    let body = py_strip_chars(&m.grp(buf, 1).unwrap_or_default());
    let items = Re::new(r"\\item(?:\[(.*?)\])?(?:\s+|(?=[\\$]))", Fl::NONE).split(&body);
    let mut res: Vec<String> = Vec::new();
    let mut idx = 1usize;
    let mut i = 1usize;
    while i < items.len() {
        let opt_tag = items[i].clone();
        let it_text = match items.get(i + 1) {
            Some(Some(v)) => py_strip(v),
            _ => String::new(),
        };
        match opt_tag {
            Some(t) if !t.is_empty() => {
                if ordered {
                    res.push(format!("{}. **{}** {}", idx, t, it_text))
                } else {
                    res.push(format!("- **{}** {}", t, it_text))
                }
            }
            _ => {
                if ordered {
                    res.push(format!("{}. {}", idx, it_text))
                } else {
                    res.push(format!("- {}", it_text))
                }
            }
        }
        idx += 1;
        i += 2;
    }
    to_chars(&format!("\n\n{}\n\n", res.join("\n")))
}

/// texmd.py:614-652 `_repl_choices`
fn repl_choices(t_in: &[char]) -> Vec<char> {
    let mut cur = t_in.to_vec();
    let pattern = Re::new(r"\\choices(?:five|four|three|six|two)?(?![a-zA-Z])", Fl::NONE);
    let labels = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'];
    let mut pos = 0usize;
    let mut out_chunks: Vec<char> = Vec::new();
    let mut last_pos = 0usize;
    loop {
        let m = match pattern.search(&cur, pos) {
            Some(m) => m,
            None => {
                out_chunks.extend(py_slice(&cur, last_pos, cur.len()));
                break;
            }
        };
        out_chunks.extend(py_slice(&cur, last_pos, m.start));
        let mut cur_pos = m.end;
        let mut opts: Vec<String> = Vec::new();
        loop {
            let mut test_pos = cur_pos;
            while test_pos < cur.len() && py_isspace(cur[test_pos]) {
                test_pos += 1;
            }
            if test_pos < cur.len() && cur[test_pos] == '{' {
                let (opt_val, np) = extract_mand_arg(&cur, test_pos);
                cur_pos = np;
                match opt_val {
                    Some(v) => opts.push(py_strip(&chars_to_string(&v))),
                    None => break,
                }
            } else {
                break;
            }
        }
        if !opts.is_empty() {
            let mut choice_lines: Vec<String> = Vec::new();
            for (idx, opt_text) in opts.iter().enumerate() {
                let lbl = if idx < labels.len() {
                    labels[idx].to_string()
                } else {
                    (idx + 1).to_string()
                };
                choice_lines.push(format!("- **{}{}** {}", lbl, ".", opt_text));
            }
            out_chunks.extend(to_chars(&format!("\n\n{}\n\n", choice_lines.join("\n"))));
        } else {
            out_chunks.extend(py_slice(&cur, m.start, cur_pos));
        }
        last_pos = cur_pos;
        pos = cur_pos;
    }
    // Python rescans from position 0 for every command; the loop above already
    // walks the whole buffer in one pass, which is equivalent because the
    // replacement text never re-introduces `\choices`.
    let _ = &mut cur;
    out_chunks
}

/// texmd.py:680-686 `_extract_caption`
fn extract_caption(body: &[char]) -> String {
    let m = Re::new(r"\\caption(?:of\{[a-zA-Z]+\})?(?:\[[^\]]*\])?\{", Fl::NONE).search(body, 0);
    if let Some(m) = m {
        let (cap_val, _) = extract_mand_arg(body, m.end - 1);
        if let Some(v) = cap_val {
            if !v.is_empty() {
                let cleaned = sub_c(&v, r"\\label\{[^}]*\}", "", Fl::NONE);
                return py_strip(&chars_to_string(&cleaned));
            }
        }
    }
    String::new()
}

/// texmd.py:716-769 `_parse_tabular`
fn parse_tabular(tbody: &[char]) -> Vec<char> {
    let rows_raw =
        Re::new(r"(?<!\\)\\\\(?:\[[^\]]*\])?", Fl::NONE).split(&py_strip_chars(tbody));
    let mut rows: Vec<Vec<char>> = Vec::new();
    for r in rows_raw.iter() {
        let rc = py_strip_chars(&to_chars(&r.clone().unwrap_or_default()));
        if !rc.is_empty() {
            rows.push(rc);
        }
    }
    let mut md_table_rows: Vec<Vec<String>> = Vec::new();
    let mut max_cols = 0usize;
    for r in rows.iter() {
        let cleaned = sub_c(
            r,
            r"\\(hline|toprule|midrule|bottomrule|cline\{[^}]*\})",
            "",
            Fl::NONE,
        );
        let cleaned = py_strip_chars(&cleaned);
        if cleaned.is_empty() {
            continue;
        }
        let raw_cells: Vec<String> = py_split_c(&cleaned, '&')
            .into_iter()
            .map(|c| py_strip(&chars_to_string(&c)))
            .collect();
        let mc = Re::new(r"\\multicolumn\{(\d+)\}\{[^}]*\}\{(.*)\}", Fl::NONE);
        let mut processed: Vec<String> = Vec::new();
        for cell in raw_cells.iter() {
            let cc = to_chars(cell);
            if let Some(m) = mc.match_(&cc) {
                let span: usize = m
                    .gs(&cc, 1)
                    .unwrap_or_default()
                    .parse()
                    .unwrap_or(1usize);
                let content = py_strip(&m.gs(&cc, 2).unwrap_or_default());
                processed.push(content);
                for _ in 0..span.saturating_sub(1) {
                    processed.push(String::new());
                }
            } else {
                processed.push(cell.clone());
            }
        }
        let dpat = Re::new(r"(?<!\\)\$", Fl::NONE);
        let mut fixed: Vec<String> = Vec::new();
        for cell in processed.iter() {
            let no_display = py_replace(cell, "$$", "");
            let d_cnt = dpat.findall(&to_chars(&no_display)).len();
            if d_cnt % 2 == 1 {
                fixed.push(format!("{}$", cell));
            } else {
                fixed.push(cell.clone());
            }
        }
        let processed = fixed;
        if processed.len() > max_cols {
            max_cols = processed.len();
        }
        md_table_rows.push(processed);
    }
    if md_table_rows.is_empty() || max_cols == 0 {
        return Vec::new();
    }
    let mut out: Vec<String> = Vec::new();
    let mut header = md_table_rows[0].clone();
    while header.len() < max_cols {
        header.push(String::new());
    }
    out.push(format!("| {} |", header.join(" | ")));
    let dashes: Vec<String> = (0..max_cols).map(|_| "---".to_string()).collect();
    out.push(format!("| {} |", dashes.join(" | ")));
    for r in md_table_rows[1..].iter() {
        let mut r = r.clone();
        while r.len() < max_cols {
            r.push(String::new());
        }
        out.push(format!("| {} |", r.join(" | ")));
    }
    to_chars(&format!("\n\n{}\n\n", out.join("\n")))
}

/// texmd.py:771-796 `_parse_tabular_blocks`
fn parse_tabular_blocks(src: &[char]) -> Vec<char> {
    let pattern = Re::new(r"\\begin\{(tabular\*?)\}(?:\[[^\]]*\])?", Fl::DOTALL);
    let mut pos = 0usize;
    let mut out: Vec<char> = Vec::new();
    let mut last_pos = 0usize;
    loop {
        let m = match pattern.search(src, pos) {
            Some(m) => m,
            None => {
                out.extend(py_slice(src, last_pos, src.len()));
                break;
            }
        };
        out.extend(py_slice(src, last_pos, m.start));
        let env_name = m.gs_or_empty(src, 1);
        let mut p_end = m.end;
        if env_name == "tabular*" {
            let (_, np) = extract_mand_arg(src, p_end);
            p_end = np;
        }
        let (_, np) = extract_mand_arg(src, p_end);
        p_end = np;
        let end_tag = format!("\\end{{{}}}", env_name);
        let et = to_chars(&end_tag);
        match find_sub(src, &et, p_end) {
            Some(end_idx) => {
                let tbody = py_slice(src, p_end, end_idx);
                out.extend(parse_tabular(&tbody));
                last_pos = end_idx + et.len();
                pos = last_pos;
            }
            None => {
                pos = p_end;
            }
        }
    }
    out
}

/// texmd.py:804-818 `_strip_caption`
fn strip_caption(src_t: &[char]) -> Vec<char> {
    let pat = Re::new(r"\\caption(?:of\{[a-zA-Z]+\})?(?:\[[^\]]*\])?\s*\{", Fl::NONE);
    let mut pos = 0usize;
    let mut out: Vec<char> = Vec::new();
    let mut last_p = 0usize;
    loop {
        let mm = match pat.search(src_t, pos) {
            Some(mm) => mm,
            None => {
                out.extend(py_slice(src_t, last_p, src_t.len()));
                break;
            }
        };
        out.extend(py_slice(src_t, last_p, mm.start));
        let (_, p_end) = extract_mand_arg(src_t, mm.end - 1);
        last_p = p_end;
        pos = p_end;
    }
    out
}

/// texmd.py:861-892 section rewriting
fn rewrite_sec(text: &[char], cmd_regex: &str, md_prefix: &str) -> Vec<char> {
    let pattern_src = format!("{}(?:\\[[^\\]]*\\])?", cmd_regex);
    let pattern = Re::new(&pattern_src, Fl::NONE);
    let mut pos = 0usize;
    let mut out: Vec<char> = Vec::new();
    let mut last_pos = 0usize;
    loop {
        let m = match pattern.search(text, pos) {
            Some(m) => m,
            None => {
                out.extend(py_slice(text, last_pos, text.len()));
                break;
            }
        };
        out.extend(py_slice(text, last_pos, m.start));
        let p_end = m.end;
        let (heading_val, p_end) = extract_mand_arg(text, p_end);
        match heading_val {
            Some(mut v) => {
                v = sub_c(&v, r"\\label\{[^}]*\}", "", Fl::NONE);
                let v = py_strip_chars(&v);
                out.extend(to_chars(&format!("\n\n{}{}\n\n", md_prefix, chars_to_string(&v))));
                last_pos = p_end;
                pos = p_end;
            }
            None => {
                out.extend(py_slice(text, m.start, p_end));
                last_pos = p_end;
                pos = p_end;
            }
        }
    }
    out
}

/// texmd.py:991-1013 `_repl_href`
fn repl_href(t_in: &[char]) -> Vec<char> {
    let pattern = Re::new(r"\\href(?![a-zA-Z])", Fl::NONE);
    let mut pos = 0usize;
    let mut out: Vec<char> = Vec::new();
    let mut last_pos = 0usize;
    loop {
        let m = match pattern.search(t_in, pos) {
            Some(m) => m,
            None => {
                out.extend(py_slice(t_in, last_pos, t_in.len()));
                break;
            }
        };
        out.extend(py_slice(t_in, last_pos, m.start));
        let mut p_end = m.end;
        let (url_val, np) = extract_mand_arg(t_in, p_end);
        p_end = np;
        let (text_val, np) = extract_mand_arg(t_in, p_end);
        p_end = np;
        match (url_val, text_val) {
            (Some(u), Some(t)) => {
                out.extend(to_chars(&format!(
                    "[{}]({})",
                    chars_to_string(&t),
                    chars_to_string(&u)
                )));
            }
            _ => {
                out.extend(py_slice(t_in, m.start, p_end));
            }
        }
        last_pos = p_end;
        pos = p_end;
    }
    out
}

/// Compatibility aliases (texmd.py:1521-1523).
pub fn latex_to_markdown(tex_content: &str, base_dir: &str) -> String {
    latex_to_md(tex_content, base_dir)
}

// ===========================================================================
// 5. Options model  (Python `Dict[str, Any]` + truthiness)
// ===========================================================================
//
// `md_to_latex()` / `build_latex_template()` receive the JS bridge's option
// dictionaries and use `x or y` plus `if not tex_opts`.  Python distinguishes
// three states that a plain `Option` cannot: key absent, key present with an
// explicit null, and key present with a falsy value (`""`, `0`, `false`,
// `{}`  ...).  `opt_get` therefore returns `Option<&OptVal>` (`None` == absent)
// and every call site decides whether the default or the stored value wins,
// exactly like `dict.get(k, default)` + `or`.

#[derive(Clone, Debug, PartialEq)]
pub enum OptVal {
    /// JSON `null` / Python `None`.
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    List(Vec<OptVal>),
    Map(OptMap),
}

/// Insertion-ordered mapping, like CPython `dict`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OptMap {
    pub entries: Vec<(String, OptVal)>,
}

static OPT_NULL: OptVal = OptVal::Null;
static OPT_EMPTY_MAP: OptVal = OptVal::Map(OptMap { entries: Vec::new() });

impl OptVal {
    /// `bool(v)` for the JSON types that can arrive over the bridge.
    pub fn truthy(&self) -> bool {
        match self {
            OptVal::Null => false,
            OptVal::Bool(b) => *b,
            OptVal::Num(n) => *n != 0.0,
            OptVal::Str(s) => !s.is_empty(),
            OptVal::List(l) => !l.is_empty(),
            OptVal::Map(m) => !m.entries.is_empty(),
        }
    }

    /// Textual form used by Python's f-string interpolation and `.startswith`.
    ///
    /// Non-strings keep a repr-like spelling; Python would raise
    /// `AttributeError` for `tex_opts.get('docClass').startswith(..)` on a
    /// number/bool, which is unreachable for well-formed bridge payloads.
    pub fn as_text(&self) -> String {
        match self {
            OptVal::Null => String::new(),
            OptVal::Bool(true) => "true".to_string(),
            OptVal::Bool(false) => "false".to_string(),
            OptVal::Num(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            OptVal::Str(s) => s.clone(),
            OptVal::List(_) => String::new(),
            OptVal::Map(_) => String::new(),
        }
    }
}

impl OptMap {
    pub fn new() -> OptMap {
        OptMap { entries: Vec::new() }
    }
}

/// `d.get(key)` — `None` means "key absent", which is NOT the same as
/// `Some(&OptVal::Null)` ("key present, value null").
pub fn opt_get<'a>(v: &'a OptVal, key: &str) -> Option<&'a OptVal> {
    match v {
        OptVal::Map(m) => m
            .entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, val)| val),
        // `tex_opts.get(...)` on a non-mapping raises in Python; treat as empty.
        _ => None,
    }
}

fn opt_text_or<'a>(v: Option<&'a OptVal>, fallback: &'a str) -> String {
    match v {
        Some(x) if x.truthy() => x.as_text(),
        _ => fallback.to_string(),
    }
}

/// `a or b or c` where `c` is the terminal default value.
fn opt_or_text(a: Option<&OptVal>, b: Option<&OptVal>, c: Option<&OptVal>, fallback: &str) -> String {
    for x in [a, b, c].iter().flatten() {
        if x.truthy() {
            return x.as_text();
        }
    }
    fallback.to_string()
}

/// Minimal JSON reader so the kernel can hand `md_to_latex` the very option
/// objects the Python app receives from the webview bridge (`options:
/// Optional[Dict]`).  Invalid input degrades to `OptVal::Null` == `options or {}`.
pub fn parse_options(json: &str) -> OptVal {
    let v: Vec<char> = json.chars().collect();
    let mut p = JsonP { s: &v, p: 0 };
    p.ws();
    match p.value(0) {
        Some(val) => {
            p.ws();
            if p.p == p.s.len() {
                val
            } else {
                OptVal::Null
            }
        }
        None => OptVal::Null,
    }
}

struct JsonP<'a> {
    s: &'a [char],
    p: usize,
}

impl<'a> JsonP<'a> {
    fn ws(&mut self) {
        // JSON whitespace is a strict subset of CPython's.
        while self.p < self.s.len() {
            match self.s[self.p] {
                ' ' | '\t' | '\n' | '\r' => self.p += 1,
                _ => break,
            }
        }
    }
    fn lit(&mut self, word: &str, val: OptVal) -> Option<OptVal> {
        let w: Vec<char> = word.chars().collect();
        if self.s.len() - self.p >= w.len() && &self.s[self.p..self.p + w.len()] == &w[..] {
            self.p += w.len();
            Some(val)
        } else {
            None
        }
    }
    fn value(&mut self, depth: usize) -> Option<OptVal> {
        if depth > 64 {
            return None;
        }
        self.ws();
        if self.p >= self.s.len() {
            return None;
        }
        match self.s[self.p] {
            '{' => {
                self.p += 1;
                let mut m = OptMap::new();
                self.ws();
                if self.p < self.s.len() && self.s[self.p] == '}' {
                    self.p += 1;
                    return Some(OptVal::Map(m));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.ws();
                    if self.p >= self.s.len() || self.s[self.p] != ':' {
                        return None;
                    }
                    self.p += 1;
                    let v = self.value(depth + 1)?;
                    m.entries.push((k, v));
                    self.ws();
                    if self.p >= self.s.len() {
                        return None;
                    }
                    match self.s[self.p] {
                        ',' => self.p += 1,
                        '}' => {
                            self.p += 1;
                            break;
                        }
                        _ => return None,
                    }
                }
                Some(OptVal::Map(m))
            }
            '[' => {
                self.p += 1;
                let mut l: Vec<OptVal> = Vec::new();
                self.ws();
                if self.p < self.s.len() && self.s[self.p] == ']' {
                    self.p += 1;
                    return Some(OptVal::List(l));
                }
                loop {
                    let v = self.value(depth + 1)?;
                    l.push(v);
                    self.ws();
                    match self.s.get(self.p)? {
                        ',' => self.p += 1,
                        ']' => {
                            self.p += 1;
                            break;
                        }
                        _ => return None,
                    }
                }
                Some(OptVal::List(l))
            }
            '"' => self.string().map(|s| OptVal::Str(s)),
            't' => self.lit("true", OptVal::Bool(true)),
            'f' => self.lit("false", OptVal::Bool(false)),
            'n' => self.lit("null", OptVal::Null),
            _ => {
                let start = self.p;
                while self.p < self.s.len()
                    && matches!(self.s[self.p], '-' | '+' | '.' | 'e' | 'E' | '0'..='9')
                {
                    self.p += 1;
                }
                if self.p == start {
                    return None;
                }
                chars_to_string(&self.s[start..self.p]).parse::<f64>().ok().map(OptVal::Num)
            }
        }
    }
    fn string(&mut self) -> Option<String> {
        if self.s.get(self.p)? != &'"' {
            return None;
        }
        self.p += 1;
        let mut out: Vec<char> = Vec::new();
        loop {
            let c = *self.s.get(self.p)?;
            self.p += 1;
            match c {
                '"' => break,
                '\\' => {
                    let e = *self.s.get(self.p)?;
                    self.p += 1;
                    match e {
                        'u' => {
                            let hex: String =
                                self.s[self.p..self.p + 4].iter().collect();
                            self.p += 4;
                            out.push(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)?);
                        }
                        'n' => out.push('\n'),
                        't' => out.push('\t'),
                        'r' => out.push('\r'),
                        'b' => out.push('\u{08}'),
                        'f' => out.push('\u{0c}'),
                        other => out.push(other),
                    }
                }
                other => out.push(other),
            }
        }
        Some(chars_to_string(&out))
    }
}

// ===========================================================================
// 6. Markdown -> LaTeX  (texmd.py:1084-1523)
// ===========================================================================

/// texmd.py:1084-1142 `LATEX_ARTICLE_TEMPLATE` — the frozen default template.
/// It is *not* `build_latex_template()`: this one has no `ctex`/bibliography
/// slots and keeps a blank line between `\begin{document}` and `\maketitle`.
pub const LATEX_ARTICLE_TEMPLATE: &str = r#"\documentclass[11pt,a4paper]{article}

% --- 核心数学与学术宏包 ---
\usepackage[utf8]{inputenc}
\usepackage[margin=2.5cm]{geometry}
\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}
\usepackage{booktabs}
\usepackage{tabularx}
\usepackage{multirow}
\usepackage{graphicx}
\usepackage{hyperref}
\usepackage{listings}
\usepackage{xcolor}
\usepackage{tcolorbox}
\usepackage{microtype}

% --- 超链接与主题色彩 ---
\hypersetup{
    colorlinks=true,
    linkcolor=blue!70!black,
    citecolor=blue!70!black,
    urlcolor=blue!70!black
}

% --- 代码块样式 ---
\lstset{
    basicstyle=\ttfamily\small,
    breaklines=true,
    frame=single,
    backgroundcolor=\color{gray!8},
    keywordstyle=\color{blue!80!black},
    commentstyle=\color{green!50!black},
    stringstyle=\color{red!70!black},
    showstringspaces=false
}

% --- 引用块与提示框 ---
\tcolorboxenvironment{quote}{
    colback=gray!5,
    colframe=gray!40,
    arc=2mm,
    left=3mm,
    right=3mm,
    top=2mm,
    bottom=2mm
}

\title{__TITLE__}
\author{__AUTHOR__}
\date{__DATE__}

\begin{document}

\maketitle

__CONTENT__

\end{document}
"#;

/// texmd.py:1145-1159 `_escape_latex_plain_text`.
///
/// Python builds `'|'.join(re.escape(k) for k in chars)`; every key is a single
/// code point, so the ordered alternation is equivalent to this per-character
/// lookup (left-to-right, non-overlapping, first hit wins).
pub fn escape_latex_plain_text(text: &str) -> String {
    const KEYS: [(char, &str); 9] = [
        ('&', r"\&"),
        ('%', r"\%"),
        ('$', r"\$"),
        ('#', r"\#"),
        ('_', r"\_"),
        ('{', r"\{"),
        ('}', r"\}"),
        ('~', r"\textasciitilde{}"),
        ('^', r"\textasciicircum{}"),
    ];
    let mut out = String::new();
    for c in text.chars() {
        match KEYS.iter().find(|(k, _)| *k == c) {
            Some((_, v)) => out.push_str(v),
            None => out.push(c),
        }
    }
    out
}

/// texmd.py:1162-1204 `_convert_inline_md_to_latex`.
pub fn convert_inline_md_to_latex(text: &str) -> String {
    let tc = to_chars(text);
    let math_tokens: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());
    let code_tokens: std::cell::RefCell<Vec<String>> = std::cell::RefCell::new(Vec::new());

    // protect inline math $...$
    let mut out = sub_fn_c(
        &tc,
        r"(?<!\\)\$([^\$]+?)\$",
        Fl::NONE,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let whole = chars_to_string(&m.grp(buf, 0).unwrap_or_default());
            let idx = {
                let mut v = math_tokens.borrow_mut();
                v.push(whole);
                v.len() - 1
            };
            to_chars(&format!("QQQMATHTOKEN{}QQQ", idx))
        },
    );

    // protect inline code `...`
    out = sub_fn_c(
        &out,
        r"`([^`]+)`",
        Fl::NONE,
        &|m: &Mx, buf: &[char]| -> Vec<char> {
            let body = m.grp(buf, 1).map(|v| chars_to_string(&v)).unwrap_or_default();
            let rep = format!("\\texttt{{{}}}", escape_latex_plain_text(&body));
            let idx = {
                let mut v = code_tokens.borrow_mut();
                v.push(rep);
                v.len() - 1
            };
            to_chars(&format!("QQQCODETOKEN{}QQQ", idx))
        },
    );

    out = sub_c(
        &out,
        r"!\[(.*?)\]\((.*?)\)",
        r"\\begin{figure}[htbp]\\centering\\includegraphics[max width=\\linewidth]{\2}\\caption{\1}\\end{figure}",
        Fl::NONE,
    );
    out = sub_c(&out, r"\[(.*?)\]\((.*?)\)", r"\\href{\2}{\1}", Fl::NONE);
    out = sub_c(&out, r"\*\*(.*?)\*\*", r"\\textbf{\1}", Fl::NONE);
    out = sub_c(&out, r"__(.*?)__", r"\\textbf{\1}", Fl::NONE);
    out = sub_c(&out, r"\*(.*?)\*", r"\\textit{\1}", Fl::NONE);
    out = sub_c(&out, r"(?<!\w)_(.*?)_{1}(?!\w)", r"\\textit{\1}", Fl::NONE);
    out = sub_c(&out, r"~~(.*?)~~", r"\\sout{\1}", Fl::NONE);

    let mut s = chars_to_string(&out);
    for (idx, ct) in code_tokens.borrow().iter().enumerate() {
        s = py_replace(&s, &format!("QQQCODETOKEN{}QQQ", idx), ct);
    }
    for (idx, mt) in math_tokens.borrow().iter().enumerate() {
        s = py_replace(&s, &format!("QQQMATHTOKEN{}QQQ", idx), mt);
    }
    s
}

fn slice_str(s: &str, start: usize, end: usize) -> String {
    chars_to_string(&py_slice(&to_chars(s), start, end))
}

/// texmd.py:1489-1518 `_render_latex_booktabs_table`.
pub fn render_latex_booktabs_table(rows: &[Vec<String>]) -> Vec<String> {
    if rows.is_empty() {
        return Vec::new();
    }
    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let col_spec = "l".repeat(col_count);
    let mut res: Vec<String> = Vec::new();
    res.push(r"\begin{table}[htbp]".to_string());
    res.push(r"\centering".to_string());
    res.push(format!("\\begin{{tabular}}{{{}}}", col_spec));
    res.push(r"\toprule".to_string());

    let mut header_cells: Vec<String> = rows[0]
        .iter()
        .map(|c| convert_inline_md_to_latex(c))
        .collect();
    while header_cells.len() < col_count {
        header_cells.push(String::new());
    }
    res.push(format!("{} \\\\", header_cells.join(" & ")));
    res.push(r"\midrule".to_string());

    for r in rows.iter().skip(1) {
        let mut cells: Vec<String> = r.iter().map(|c| convert_inline_md_to_latex(c)).collect();
        while cells.len() < col_count {
            cells.push(String::new());
        }
        res.push(format!("{} \\\\", cells.join(" & ")));
    }

    res.push(r"\bottomrule".to_string());
    res.push(r"\end{tabular}".to_string());
    res.push(r"\end{table}".to_string());
    res
}

/// texmd.py:1207-1281 `build_latex_template`.
pub fn build_latex_template(options: &OptVal) -> String {
    let opts: &OptVal = if options.truthy() { options } else { &OPT_EMPTY_MAP };
    let tex_opts = or3_opt(opt_get(opts, "tex"), opt_get(opts, "latex"));

    let doc_class = opt_text_or(opt_get(tex_opts, "docClass"), "article");
    let font_size = opt_text_or(opt_get(tex_opts, "fontSize"), "11pt");
    let paper_size = opt_text_or(opt_get(tex_opts, "paperSize"), "a4paper");
    // `not tex_opts` -> the empty dict also selects the *True* default.
    let use_ctex = match opt_get(tex_opts, "useCtex") {
        Some(v) => v.truthy(),
        None => !matches!(tex_opts, OptVal::Map(m) if m.entries.is_empty()),
    };
    let margin = opt_text_or(opt_get(tex_opts, "margin"), "2.5cm");
    let bib_engine = opt_text_or(opt_get(tex_opts, "bibEngine"), "biblatex");

    let ctex_pkg = if use_ctex && !doc_class.starts_with("ctex") {
        r"\usepackage{ctex}".to_string()
    } else {
        String::new()
    };
    let bib_pkg = if bib_engine == "biblatex" {
        r"\usepackage[backend=biber,style=numeric]{biblatex}".to_string()
    } else if bib_engine == "natbib" {
        r"\usepackage{natbib}".to_string()
    } else {
        String::new()
    };

    format!(
        "\\documentclass[{font_size},{paper_size}]{{{doc_class}}}

% --- 核心数学与学术宏包 ---
\\usepackage[utf8]{{inputenc}}
\\usepackage[margin={margin}]{{geometry}}
\\usepackage{{amsmath,amssymb,amsfonts,amsthm,mathtools}}
\\usepackage{{booktabs}}
\\usepackage{{tabularx}}
\\usepackage{{multirow}}
\\usepackage{{graphicx}}
\\usepackage{{hyperref}}
\\usepackage{{listings}}
\\usepackage{{xcolor}}
\\usepackage{{tcolorbox}}
\\usepackage{{microtype}}
{ctex_pkg}
{bib_pkg}

% --- 超链接与主题色彩 ---
\\hypersetup{{
    colorlinks=true,
    linkcolor=blue!70!black,
    citecolor=blue!70!black,
    urlcolor=blue!70!black
}}

% --- 代码块样式 ---
\\lstset{{
    basicstyle=\\ttfamily\\small,
    breaklines=true,
    frame=single,
    backgroundcolor=\\color{{gray!8}},
    keywordstyle=\\color{{blue!80!black}},
    commentstyle=\\color{{green!50!black}},
    stringstyle=\\color{{red!70!black}},
    showstringspaces=false
}}

% --- 引用块与提示框 ---
\\tcolorboxenvironment{{quote}}{{
    colback=gray!5,
    colframe=gray!40,
    arc=2mm,
    left=3mm,
    right=3mm,
    top=2mm,
    bottom=2mm
}}

\\title{{__TITLE__}}
\\author{{__AUTHOR__}}
\\date{{__DATE__}}

\\begin{{document}}
\\maketitle

__CONTENT__

\\end{{document}}
"
    )
}

fn or3_opt<'a>(a: Option<&'a OptVal>, b: Option<&'a OptVal>) -> &'a OptVal {
    if let Some(v) = a {
        if v.truthy() {
            return v;
        }
    }
    if let Some(v) = b {
        if v.truthy() {
            return v;
        }
    }
    &OPT_EMPTY_MAP
}

/// texmd.py:1284-1486 `md_to_latex`.
pub fn md_to_latex(
    md_content: &str,
    title: &str,
    author: &str,
    standalone: bool,
    options: &OptVal,
) -> String {
    let lines = py_splitlines(md_content);
    let mut latex_lines: Vec<String> = Vec::new();

    let opts: &OptVal = if options.truthy() { options } else { &OPT_EMPTY_MAP };
    let tex_opts = or3_opt(opt_get(opts, "tex"), opt_get(opts, "latex"));

    let mut doc_title = opt_or_text(
        opt_get(tex_opts, "title"),
        opt_get(opt_get(opts, "meta").unwrap_or(&OPT_EMPTY_MAP), "title"),
        None,
        title,
    );
    let mut doc_author = opt_or_text(
        opt_get(tex_opts, "author"),
        opt_get(opt_get(opts, "meta").unwrap_or(&OPT_EMPTY_MAP), "author"),
        None,
        author,
    );
    let mut doc_date = r"\today".to_string();
    let mut content_start_idx: usize = 0;

    // 1. YAML frontmatter
    if lines.len() > 2 && py_strip(&lines[0]) == "---" {
        let mut i = 1usize;
        while i < lines.len() {
            if py_strip(&lines[i]) == "---" {
                content_start_idx = i + 1;
                break;
            }
            let fm_line = lines[i].clone();
            if contains_str(&to_chars(&fm_line), ":") {
                let fc = to_chars(&fm_line);
                let colon = find_sub(&fc, &to_chars(":"), 0).unwrap_or(0);
                let k = py_strip(&chars_to_string(&py_slice(&fc, 0, colon))).to_lowercase();
                let vraw = chars_to_string(&py_slice(&fc, colon + 1, fc.len()));
                let v = py_strip_set(
                    &to_chars(&py_strip(&vraw)),
                    &|c: char| c == '"' || c == '\'',
                );
                let v = chars_to_string(&v);
                if k == "title" {
                    doc_title = v;
                } else if k == "author" || k == "authors" {
                    doc_author = v;
                } else if k == "date" {
                    doc_date = v;
                }
            }
            i += 1;
        }
    }

    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_buffer: Vec<String> = Vec::new();
    let mut in_table = false;
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut in_display_math = false;
    let mut math_buffer: Vec<String> = Vec::new();

    let hr_re = Re::new(r"^(\*{3,}|-{3,}|_{3,})$", Fl::NONE);
    let ul_re = Re::new(r"^[\*\-\+]\s+", Fl::NONE);
    let ol_re = Re::new(r"^\d+\.\s+", Fl::NONE);
    let sep_re = Re::new(r"^[:\-\s|]+$", Fl::NONE);
    let head_re = Re::new(r"^(#{1,6})\s+(.*)$", Fl::NONE);

    let mut i = content_start_idx;
    while i < lines.len() {
        let line = lines[i].clone();
        let stripped = py_strip(&line);

        // 1. fenced code
        if starts_with(&to_chars(&stripped), "```") {
            if in_code_block {
                let mut head = r"\begin{lstlisting}".to_string();
                if !code_lang.is_empty() {
                    head.push_str(&format!("[language={}]", code_lang));
                }
                latex_lines.push(head);
                latex_lines.extend(code_buffer.iter().cloned());
                latex_lines.push(r"\end{lstlisting}".to_string());
                in_code_block = false;
                code_buffer = Vec::new();
                code_lang = String::new();
            } else {
                in_code_block = true;
                let sc = to_chars(&stripped);
                code_lang = py_strip(&chars_to_string(&py_slice(&sc, 3, sc.len())));
                code_buffer = Vec::new();
            }
            i += 1;
            continue;
        }

        if in_code_block {
            code_buffer.push(line);
            i += 1;
            continue;
        }

        // 2. display math $$ ... $$
        if starts_with(&to_chars(&stripped), "$$") {
            if in_display_math {
                math_buffer.push(line);
                let math_body =
                    math_body_normalize(&math_buffer.join("\n"), false);
                latex_lines.push(wrap_equation_star(&math_body));
                in_display_math = false;
                math_buffer = Vec::new();
            } else {
                let sc = to_chars(&stripped);
                let ends = ends_with(&sc, "$$");
                if ends && sc.len() > 2 {
                    let math_body = py_strip(&chars_to_string(&py_slice(&sc, 2, sc.len() - 2)));
                    latex_lines.push(wrap_equation_star(&math_body));
                } else {
                    in_display_math = true;
                    math_buffer = vec![line];
                }
            }
            i += 1;
            continue;
        }

        if in_display_math {
            if ends_with(&to_chars(&stripped), "$$") {
                math_buffer.push(line);
                let math_body =
                    math_body_normalize(&math_buffer.join("\n"), true);
                latex_lines.push(wrap_equation_star(&math_body));
                in_display_math = false;
                math_buffer = Vec::new();
            } else {
                math_buffer.push(line);
            }
            i += 1;
            continue;
        }

        // 3. pipe tables
        let sc = to_chars(&stripped);
        if starts_with(&sc, "|") && ends_with(&sc, "|") {
            let inner = if sc.len() >= 2 {
                chars_to_string(&py_slice(&sc, 1, sc.len() - 1))
            } else {
                String::new()
            };
            let cells: Vec<String> = py_split_c(&to_chars(&inner), '|')
                .into_iter()
                .map(|c| py_strip(&chars_to_string(&c)))
                .collect();
            if !sep_re.is_match(&sc) {
                if !in_table {
                    in_table = true;
                    table_rows = Vec::new();
                }
                table_rows.push(cells);
            }
            i += 1;
            continue;
        } else if in_table {
            latex_lines.extend(render_latex_booktabs_table(&table_rows));
            in_table = false;
            table_rows = Vec::new();
        }

        // 4. headings
        if let Some(m) = head_re.match_(&to_chars(&line)) {
            let hashes = m.grp(&to_chars(&line), 1).unwrap_or_default();
            let level = hashes.len();
            let raw = chars_to_string(&m.grp(&to_chars(&line), 2).unwrap_or_default());
            let htext = convert_inline_md_to_latex(&py_strip(&raw));
            let cmd = match level {
                1 => r"\section",
                2 => r"\subsection",
                3 => r"\subsubsection",
                4 => r"\paragraph",
                5 => r"\subparagraph",
                6 => r"\textbf",
                _ => r"\paragraph",
            };
            latex_lines.push(format!("\n{}{{{}}}", cmd, htext));
            i += 1;
            continue;
        }

        // 5. blockquote
        if starts_with(&sc, ">") {
            let rest = chars_to_string(&py_slice(&sc, 1, sc.len()));
            let quote_text = convert_inline_md_to_latex(&py_strip(&rest));
            latex_lines.push(r"\begin{quote}".to_string());
            latex_lines.push(quote_text);
            latex_lines.push(r"\end{quote}".to_string());
            i += 1;
            continue;
        }

        // 6. unordered list
        if ul_re.is_match(&to_chars(&stripped)) {
            let usc = to_chars(&stripped);
            let item_text = convert_inline_md_to_latex(&chars_to_string(&sub_c(
                &usc,
                r"^[\*\-\+]\s+",
                "",
                Fl::NONE,
            )));
            latex_lines.push(r"\begin{itemize}".to_string());
            latex_lines.push(format!("  \\item {}", item_text));
            while i + 1 < lines.len()
                && ul_re.is_match(&to_chars(&py_strip(&lines[i + 1])))
            {
                i += 1;
                let nsc = to_chars(&py_strip(&lines[i]));
                let next_item = convert_inline_md_to_latex(&chars_to_string(&sub_c(
                    &nsc,
                    r"^[\*\-\+]\s+",
                    "",
                    Fl::NONE,
                )));
                latex_lines.push(format!("  \\item {}", next_item));
            }
            latex_lines.push(r"\end{itemize}".to_string());
            i += 1;
            continue;
        }

        // 7. ordered list
        if ol_re.is_match(&to_chars(&stripped)) {
            let nsc = to_chars(&stripped);
            let item_text = convert_inline_md_to_latex(&chars_to_string(&sub_c(
                &nsc,
                r"^\d+\.\s+",
                "",
                Fl::NONE,
            )));
            latex_lines.push(r"\begin{enumerate}".to_string());
            latex_lines.push(format!("  \\item {}", item_text));
            while i + 1 < lines.len()
                && ol_re.is_match(&to_chars(&py_strip(&lines[i + 1])))
            {
                i += 1;
                let msc = to_chars(&py_strip(&lines[i]));
                let next_item = convert_inline_md_to_latex(&chars_to_string(&sub_c(
                    &msc,
                    r"^\d+\.\s+",
                    "",
                    Fl::NONE,
                )));
                latex_lines.push(format!("  \\item {}", next_item));
            }
            latex_lines.push(r"\end{enumerate}".to_string());
            i += 1;
            continue;
        }

        // 8. horizontal rule
        if hr_re.is_match(&sc) {
            latex_lines.push(r"\noindent\rule{\textwidth}{0.4pt}".to_string());
            i += 1;
            continue;
        }

        // 9. paragraph body
        if !stripped.is_empty() {
            latex_lines.push(convert_inline_md_to_latex(&line));
        } else {
            latex_lines.push(String::new());
        }
        i += 1;
    }

    if in_table && !table_rows.is_empty() {
        latex_lines.extend(render_latex_booktabs_table(&table_rows));
    }

    let content_latex = latex_lines.join("\n");

    if standalone {
        let tpl = build_latex_template(options);
        let r1 = py_replace(&tpl, "__TITLE__", &doc_title);
        let r2 = py_replace(&r1, "__AUTHOR__", &doc_author);
        let r3 = py_replace(&r2, "__DATE__", &doc_date);
        py_replace(&r3, "__CONTENT__", &content_latex)
    } else {
        content_latex
    }
}

/// `'\n'.join(math_buffer).strip('$').strip()` (and the rstrip-first variant).
fn math_body_normalize(joined: &str, rstrip_first: bool) -> String {
    let dollar = |c: char| c == '$';
    let v = if rstrip_first {
        let a = py_rstrip_set(&to_chars(joined), &dollar);
        py_strip_set(&a, &dollar)
    } else {
        py_strip_set(&to_chars(joined), &dollar)
    };
    py_strip(&chars_to_string(&v))
}

/// `\begin{equation*}` wrapper unless the body already looks like an env.
fn wrap_equation_star(math_body: &str) -> String {
    let b = to_chars(math_body);
    if starts_with(&b, r"\begin{") && ends_with(&b, r"\end{") {
        math_body.to_string()
    } else {
        format!("\\begin{{equation*}}\n{}\n\\end{{equation*}}", math_body)
    }
}

/// texmd.py:1284 with every default in place.
pub fn md_to_latex_default(md_content: &str) -> String {
    md_to_latex(md_content, "Academic Document", "", true, &OptVal::Null)
}

/// Compatibility alias (texmd.py:1523).
pub fn markdown_to_latex(
    md_content: &str,
    title: &str,
    author: &str,
    standalone: bool,
    options: &OptVal,
) -> String {
    md_to_latex(md_content, title, author, standalone, options)
}


// >>> BEGIN GENERATED PARITY TESTS (scratch/rust_parity/tx1/gen_tests.py)
// ===========================================================================
// Parity tests.  Every expected literal in this module was captured from
// readmd_modules/texmd.py at porting time (scratch/rust_parity/tx1/diff_gen*.py
// + cases/expected/mdcases/mdexpected), not from the Rust behaviour.
// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tex2md_ack() {
        assert_eq!(
            latex_to_md("\\section*{Acknowledgements}\nThanks.", ""),
            "## \u{81f4}\u{8c22} (Acknowledgements)\nThanks.",
        );
    }

    #[test]
    fn tex2md_algo() {
        assert_eq!(
            latex_to_md("\\begin{algorithm}\n\\caption{Al}\n\\begin{algorithmic}\n\\STATE x\n\\end{algorithmic}\n\\end{algorithm}", ""),
            "**\u{7b97}\u{6cd5}\u{ff1a}Al**\n\n```pseudocode\n  x\n```",
        );
    }

    #[test]
    fn tex2md_appendix() {
        assert_eq!(
            latex_to_md("\\appendix\n\\section{App}", ""),
            "# \u{9644}\u{5f55} (Appendix)\n\n# App",
        );
    }

    #[test]
    fn tex2md_bgroup() {
        assert_eq!(
            latex_to_md("{\\bf Bold} and {\\sc Loos} and {\\it Ital}", ""),
            "**Bold** and <span style=\"font-variant: small-caps;\">Loos</span> and *Ital*",
        );
    }

    #[test]
    fn tex2md_bib() {
        assert_eq!(
            latex_to_md("\\begin{thebibliography}{9}\n\\bibitem{a1} Author. Title. 2020.\n\\bibitem{b2} Second.\n\\end{thebibliography}", ""),
            "## \u{53c2}\u{8003}\u{6587}\u{732e} (References)\n\n- **[@a1]** Author. Title. 2020.\n- **[@b2]** Second.",
        );
    }

    #[test]
    fn tex2md_blank() {
        assert_eq!(
            latex_to_md("   \n\t \n ", ""),
            "",
        );
    }

    #[test]
    fn tex2md_center_env() {
        assert_eq!(
            latex_to_md("\\begin{center}\nmid\n\\end{center}", ""),
            "mid",
        );
    }

    #[test]
    fn tex2md_choices() {
        assert_eq!(
            latex_to_md("\\begin{eqvariables}\n\\begin{choices}\n\\Five 5\n\\Six 6\n\\end{choices}\n\\end{eqvariables}", ""),
            "\\begin{eqvariables}\n\\begin{choices}\n\\Five 5\n\\Six 6\n\\end{choices}\n\\end{eqvariables}",
        );
    }

    #[test]
    fn tex2md_circlelist() {
        assert_eq!(
            latex_to_md("\\begin{circlelist}\n\\item a\n\\item b\n\\end{circlelist}", ""),
            "- **\u{2460}** a\n- **\u{2461}** b",
        );
    }

    #[test]
    fn tex2md_cite() {
        assert_eq!(
            latex_to_md("\\cite{a, b} and \\citep{x}", ""),
            "[@a; @b] and [@x]",
        );
    }

    #[test]
    fn tex2md_comment_only() {
        assert_eq!(
            latex_to_md("% just a comment\n", ""),
            "",
        );
    }

    #[test]
    fn tex2md_description() {
        assert_eq!(
            latex_to_md("\\begin{description}\n\\item[Foo] bar\n\\end{description}", ""),
            "- **Foo** bar",
        );
    }

    #[test]
    fn tex2md_doc1() {
        assert_eq!(
            latex_to_md("% a comment\n\\documentclass{article}\n\\usepackage{amsmath}\n\\newcommand{\\f}[2]{#1^#2}\n\\title{My Title}\n\\author{A \\and B}\n\\date{2024}\n\\begin{document}\n\\begin{abstract}\nHello abstract.\n\\end{abstract}\n\\section{Intro}\nText with \\f{x}{2} and inline $a_b$ and display\n\\[\n  y = \\alpha + \\beta\n\\]\n\\begin{itemize}\n  \\item alpha\n  \\item beta\n\\end{itemize}\n\\begin{enumerate}\n  \\item x\n  \\item y\n\\end{enumerate}\n\\begin{table}[htbp]\n\\centering\n\\caption{Cap}\n\\begin{tabular}{lcr}\n\\toprule\na & b & c \\\\\n\\midrule\nd & e & f \\\\\n\\bottomrule\n\\end{tabular}\n\\end{table}\nSee \\ref{intro} and \\cite{ref1, ref2} and \\cite[p.5]{x3}.\n\\footnote{Note one}\n\\begin{figure}[h]\n\\includegraphics[width=0.5\\textwidth]{a.png}\n\\caption{FigCap}\n\\end{figure}\n\\subsection{Sub}\nBold \\textbf{b} ital \\textit{i} tt \\texttt{t} ul \\underline{u} strike \\sout{s}.\n100\\% done \\& cost \\$5 \\# hash \\_ underscore \\{brace\\} a~b.\n\\begin{equation}\\label{eq:x}\ne^{i\\pi} + 1 = 0\n\\end{equation}\n\\begin{theorem}[Foo]\\label{x}\nBody\\\\line2\n\\end{theorem}\n\\begin{verbatim}\nraw & text\n\\end{verbatim}\n\\begin{lstlisting}[language=Python]\nx = 1\n\\end{lstlisting}\n\\begin{tabular}{|c|c|}\n\\hline\n1 & 2 \\\\ \\hline\n3 & 4 \\\\ \\hline\n\\end{tabular}\n\\begin{tabular}{ccc}\na & b \\\\\n\\end{tabular}\n\\begin{tabular}{lll}\na & b & \\\\\n\\end{tabular}\n\\begin{tabular}{cc}\n$a$ & $$b$$ \\\\\nc$$ & d \\\\\n\\end{tabular}\n\\begin{tabular}{cc}\na\\textbackslash b & c \\\\\n\\end{tabular}\n\\begin{tabular}{cc}\n\\multicolumn{2}{c}{wide} \\\\\na & b \\\\\n\\end{tabular}\n\u{4e2d}\u{6587}\u{6d4b}\u{8bd5} \\textbf{\u{7c97}\u{4f53}} $\\alpha$\n\\end{document}\n% trailing comment\n", ""),
            "---\ntitle: \"My Title\"\nauthor: \"A & B\"\ndate: \"2024\"\n---\n\n\n> **\u{6458}\u{8981} (Abstract)**\n>\n> Hello abstract.\n\n# Intro\n\nText with x^2 and inline $a_b$ and display\n\n$$\n\n  y = \\alpha + \\beta\n\n$$\n\n- alpha\n- beta\n\n1. x\n2. y\n\n**\u{8868}\u{ff1a}Cap**\n| a | b | c |\n| --- | --- | --- |\n| d | e | f |\nSee [intro] and [@ref1; @ref2] and [@x3].\n[^1]\n\n![FigCap](a.png)\n\n## Sub\n\nBold **b** ital *i* tt `t` ul <u>u</u> strike   s  .\n100% done & cost \\$5 # hash _ underscore {brace} a b.\n\n$$\ne^{i\\pi} + 1 = 0\n$$\n\n> **\u{5b9a}\u{7406} (Theorem) (Foo)**\n>\n> \\label{x}\n> Body\\\\line2\n\n```\n\nraw & text\n\n```\n\n```python\nx = 1\n```\n\n| 1 | 2 |\n| --- | --- |\n| 3 | 4 |\n\n| a | b |\n| --- | --- |\n\n| a | b |  |\n| --- | --- | --- |\n\n| $a$ | $$b$$ |\n| --- | --- |\n| c$$ | d |\n\n| a\\ b | c |\n| --- | --- |\n\n| wide |  |\n| --- | --- |\n| a | b |\n\n\u{4e2d}\u{6587}\u{6d4b}\u{8bd5} **\u{7c97}\u{4f53}** $\\alpha$\n\n---\n\n### \u{811a}\u{6ce8} (Footnotes)\n\n[^1]: Note one",
        );
    }

    #[test]
    fn tex2md_empty() {
        assert_eq!(
            latex_to_md("", ""),
            "",
        );
    }

    #[test]
    fn tex2md_enumerate() {
        assert_eq!(
            latex_to_md("\\begin{enumerate}\n\\item x\n\\item y\n\\end{enumerate}", ""),
            "1. x\n2. y",
        );
    }

    #[test]
    fn tex2md_exam() {
        assert_eq!(
            latex_to_md("\\begin{problem}\nQ1?\n\\end{problem}\n\\begin{solution}\nS1.\n\\end{solution}", ""),
            "#### \u{3010}\u{9898}\u{76ee}\u{3011}\n\nQ1?\n\n> **\u{3010}\u{89e3}\u{6790}\u{3011}**\n>\n> S1.",
        );
    }

    #[test]
    fn tex2md_figure() {
        assert_eq!(
            latex_to_md("\\begin{figure}\n\\includegraphics{a.png}\n\\caption{Cap}\n\\end{figure}", ""),
            "![Cap](a.png)",
        );
    }

    #[test]
    fn tex2md_footnote_ref() {
        assert_eq!(
            latex_to_md("a\\footnote{x} b\\footnote{y}", ""),
            "a[^1] b[^2]\n\n---\n\n### \u{811a}\u{6ce8} (Footnotes)\n\n[^1]: x\n[^2]: y",
        );
    }

    #[test]
    fn tex2md_hr() {
        assert_eq!(
            latex_to_md("a\\hrule b and \\rule{1cm}{0.4pt} c", ""),
            "a\n\n---\n\n b and \n\n---\n\n c",
        );
    }

    #[test]
    fn tex2md_href() {
        assert_eq!(
            latex_to_md("\\href{http://x.y}{Link} and \\url{http://z.w}", ""),
            "[Link](http://x.y) and <http://z.w>",
        );
    }

    #[test]
    fn tex2md_input_missing() {
        assert_eq!(
            latex_to_md("\\input{nope}", ""),
            "\\input{nope}",
        );
    }

    #[test]
    fn tex2md_item_fallback() {
        assert_eq!(
            latex_to_md("alpha and \\item beta", ""),
            "alpha and - beta",
        );
    }

    #[test]
    fn tex2md_keywords() {
        assert_eq!(
            latex_to_md("\\noindent\\textbf{Keywords:} a, b, c", ""),
            "**Keywords:** a, b, c",
        );
    }

    #[test]
    fn tex2md_long_preamble() {
        assert_eq!(
            latex_to_md("\\def\\z{Z}\n\\renewcommand{\\y}[1]{y#1}\n\\DeclareMathOperator{\\lcm}{lcm}\n\\begin{document}\n\\z \\y{a} \\lcm(2,3)\n\\end{document}", ""),
            "Z ya \\operatorname{lcm}(2,3)",
        );
    }

    #[test]
    fn tex2md_lst() {
        assert_eq!(
            latex_to_md("\\begin{lstlisting}[language=Python]\nx = 1\n\\end{lstlisting}", ""),
            "```python\nx = 1\n```",
        );
    }

    #[test]
    fn tex2md_math_inline() {
        assert_eq!(
            latex_to_md("a $b_c$ d $$e$$ f \\(g\\) h \\[i\\] j", ""),
            "a $b_c$ d $$e$$ f $g$ h \n\n$$\ni\n$$\n\n j",
        );
    }

    #[test]
    fn tex2md_minipage() {
        assert_eq!(
            latex_to_md("\\begin{minipage}{0.5\\textwidth}\ninside\n\\end{minipage}", ""),
            "\\begin{minipage}{0.5\\textwidth}\ninside\n\\end{minipage}",
        );
    }

    #[test]
    fn tex2md_nested_fmt() {
        assert_eq!(
            latex_to_md("\\textbf{a \\emph{b} c \\footnote{f}}", ""),
            "**a *b* c [^1]**\n\n---\n\n### \u{811a}\u{6ce8} (Footnotes)\n\n[^1]: f",
        );
    }

    #[test]
    fn tex2md_nested_lists() {
        assert_eq!(
            latex_to_md("\\begin{itemize}\n\\item a\n\\begin{itemize}\n\\item b\n\\end{itemize}\n\\end{itemize}", ""),
            "- a\n\\begin{itemize}\n- b\n\n\\end{itemize}",
        );
    }

    #[test]
    fn tex2md_newline_cmd() {
        assert_eq!(
            latex_to_md("a\\\\b and \\newline c and \\linebreak d", ""),
            "a\\\\b and \n c and \n d",
        );
    }

    #[test]
    fn tex2md_only_math() {
        assert_eq!(
            latex_to_md("\\begin{align}\na &= b \\\\\nc &= d\n\\end{align}", ""),
            "$$\n\\begin{align}\na &= b \\\\\nc &= d\n\\end{align}\n$$",
        );
    }

    #[test]
    fn tex2md_quote_env() {
        assert_eq!(
            latex_to_md("\\begin{quote}\nquoted\n\\end{quote}", ""),
            "> quoted",
        );
    }

    #[test]
    fn tex2md_quotes() {
        assert_eq!(
            latex_to_md("``Hi'' and `lo' and \\% 50 \\# \\& \\_ and a~b", ""),
            "\u{201c}Hi\u{201d} and \u{2018}lo\u{2019} and % 50 # & _ and a b",
        );
    }

    #[test]
    fn tex2md_recursion_macro() {
        assert_eq!(
            latex_to_md("\\newcommand{\\L}{\\L!}\n\\L", ""),
            "\\L!!!!!",
        );
    }

    #[test]
    fn tex2md_section_none() {
        assert_eq!(
            latex_to_md("\\section*{S}\n body", ""),
            "# S\n\n body",
        );
    }

    #[test]
    fn tex2md_subsup() {
        assert_eq!(
            latex_to_md("$x^2_i$ and \\(y^{ab}\\)", ""),
            "$x^2_i$ and $y^{ab}$",
        );
    }

    #[test]
    fn tex2md_tab_cjk() {
        assert_eq!(
            latex_to_md("\\begin{tabular}{cc}\u{4e2d}\u{6587}\u{6d4b}\u{8bd5} & \u{4e59} \\\\ \u{7532} & \u{4e19}\\end{tabular}", ""),
            "| \u{4e2d}\u{6587}\u{6d4b}\u{8bd5} | \u{4e59} |\n| --- | --- |\n| \u{7532} | \u{4e19} |",
        );
    }

    #[test]
    fn tex2md_tab_empty() {
        assert_eq!(
            latex_to_md("\\begin{tabular}{cc}\\end{tabular}", ""),
            "",
        );
    }

    #[test]
    fn tex2md_tab_esc() {
        assert_eq!(
            latex_to_md("\\begin{tabular}{cc}a\\textbackslash b & c \\\\ d & e\\end{tabular}", ""),
            "| a\\ b | c |\n| --- | --- |\n| d | e |",
        );
    }

    #[test]
    fn tex2md_tab_hlines() {
        assert_eq!(
            latex_to_md("\\begin{tabular}{ccc}\\hline a & b  c & d \\\\ \\hline\\end{tabular}", ""),
            "| a | b  c | d |\n| --- | --- | --- |",
        );
    }

    #[test]
    fn tex2md_tab_math() {
        assert_eq!(
            latex_to_md("\\begin{tabular}{cc}$a$ & $$b$$ \\\\ c$$ & d\\end{tabular}", ""),
            "| $a$ | $$b$$ |\n| --- | --- |\n| c$$ | d |",
        );
    }

    #[test]
    fn tex2md_tab_multi() {
        assert_eq!(
            latex_to_md("\\begin{tabular}{ccc}\\multicolumn{2}{c}{wide} \\\\ a & b & c\\end{tabular}", ""),
            "| wide |  |  |\n| --- | --- | --- |\n| a | b | c |",
        );
    }

    #[test]
    fn tex2md_tab_resizebox() {
        assert_eq!(
            latex_to_md("\\begin{tabular}{cc}\\resizebox{\\textwidth}{!}{a & b \\\\ c & d}\\end{tabular}", ""),
            "| a | b |\n| --- | --- |\n| c | d |",
        );
    }

    #[test]
    fn tex2md_tab_simple() {
        assert_eq!(
            latex_to_md("\\begin{tabular}{cc}a & b \\\\ c & d\\end{tabular}", ""),
            "| a | b |\n| --- | --- |\n| c | d |",
        );
    }

    #[test]
    fn tex2md_tab_trailing() {
        assert_eq!(
            latex_to_md("\\begin{tabular}{cccc}a & b &  & \\end{tabular}", ""),
            "| a | b |  |  |\n| --- | --- | --- | --- |",
        );
    }

    #[test]
    fn tex2md_tab_uneven() {
        assert_eq!(
            latex_to_md("\\begin{tabular}{cc}a & b \\\\ c & d & e\\end{tabular}", ""),
            "| a | b |  |\n| --- | --- | --- |\n| c | d | e |",
        );
    }

    #[test]
    fn tex2md_table_env() {
        assert_eq!(
            latex_to_md("\\begin{table}\n\\caption{Cap}\n\\begin{tabular}{cc}a & b\\\\ c & d\n\\end{tabular}\n\\end{table}", ""),
            "**\u{8868}\u{ff1a}Cap**\n| a | b |\n| --- | --- |\n| c | d |",
        );
    }

    #[test]
    fn tex2md_texesc() {
        assert_eq!(
            latex_to_md("\\textbar\\textless\\textgreater\\textasciicircum{} \\textasciitilde{}", ""),
            "|<>^{}  {}",
        );
    }

    #[test]
    fn tex2md_textsc_math() {
        assert_eq!(
            latex_to_md("$\\textsc{Foo}$ and \\textsc{Bar}", ""),
            "$\\mathrm{Foo}$ and <span style=\"font-variant: small-caps;\">Bar</span>",
        );
    }

    #[test]
    fn tex2md_thanks() {
        assert_eq!(
            latex_to_md("\\title{T\\thanks{hi}}\n\\author{A}", ""),
            "---\ntitle: \"T\"\nauthor: \"A\"\n---\n\n\n\\title{T\\thanks{hi}}\n\\author{A}",
        );
    }

    #[test]
    fn tex2md_theorem() {
        assert_eq!(
            latex_to_md("\\begin{theorem}[Foo]\\label{x}Body\\\\line2\n\\end{theorem}", ""),
            "> **\u{5b9a}\u{7406} (Theorem) (Foo)**\n>\n> \\label{x}Body\\\\line2",
        );
    }

    #[test]
    fn tex2md_verb() {
        assert_eq!(
            latex_to_md("\\verb|a+b| and \\verb*{q}", ""),
            "`a+b` and \\verb*{q}",
        );
    }

    #[test]
    fn md2tex_blank_lines() {
        assert_eq!(
            md_to_latex("a\n\n\nb", "Academic Document", "", false, &parse_options("null")),
            "a\n\n\nb",
        );
    }

    #[test]
    fn md2tex_cjk() {
        assert_eq!(
            md_to_latex("# \u{4e2d}\u{6587}\u{6807}\u{9898}\n\n\u{6b63}\u{6587} **\u{7c97}\u{4f53}** \u{4e0e} $\\alpha$\u{3002}", "Academic Document", "", false, &parse_options("null")),
            "\n\\section{\u{4e2d}\u{6587}\u{6807}\u{9898}}\n\n\u{6b63}\u{6587} \\textbf{\u{7c97}\u{4f53}} \u{4e0e} $\\alpha$\u{3002}",
        );
    }

    #[test]
    fn md2tex_cjk_standalone() {
        assert_eq!(
            md_to_latex("\u{6b63}\u{6587} \u{4e2d}\u{6587}", "Academic Document", "", true, &parse_options("null")),
            "\\documentclass[11pt,a4paper]{article}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\n\\usepackage[backend=biber,style=numeric]{biblatex}\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{Academic Document}\n\\author{}\n\\date{\\today}\n\n\\begin{document}\n\\maketitle\n\n\u{6b63}\u{6587} \u{4e2d}\u{6587}\n\n\\end{document}\n",
        );
    }

    #[test]
    fn md2tex_code_math_inside() {
        assert_eq!(
            md_to_latex("```math\n$$x$$\n```", "Academic Document", "", false, &parse_options("null")),
            "\\begin{lstlisting}[language=math]\n$$x$$\n\\end{lstlisting}",
        );
    }

    #[test]
    fn md2tex_code_nolang() {
        assert_eq!(
            md_to_latex("```\nx\n```", "Academic Document", "", false, &parse_options("null")),
            "\\begin{lstlisting}\nx\n\\end{lstlisting}",
        );
    }

    #[test]
    fn md2tex_code_token_clash() {
        assert_eq!(
            md_to_latex("QQQMATHTOKEN0QQQ and $x$", "Academic Document", "", false, &parse_options("null")),
            "$x$ and $x$",
        );
    }

    #[test]
    fn md2tex_crlf() {
        assert_eq!(
            md_to_latex("a\r\n# H\r\nb", "Academic Document", "", false, &parse_options("null")),
            "a\n\n\\section{H}\nb",
        );
    }

    #[test]
    fn md2tex_empty() {
        assert_eq!(
            md_to_latex("", "Academic Document", "", true, &parse_options("null")),
            "\\documentclass[11pt,a4paper]{article}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\n\\usepackage[backend=biber,style=numeric]{biblatex}\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{Academic Document}\n\\author{}\n\\date{\\today}\n\n\\begin{document}\n\\maketitle\n\n\n\n\\end{document}\n",
        );
    }

    #[test]
    fn md2tex_esc_dollar() {
        assert_eq!(
            md_to_latex("price \\$5 and $x$", "Academic Document", "", false, &parse_options("null")),
            "price \\$5 and $x$",
        );
    }

    #[test]
    fn md2tex_fm_date() {
        assert_eq!(
            md_to_latex("---\ndate: 2020\ntitle: X\n---\nb", "Academic Document", "", false, &parse_options("null")),
            "b",
        );
    }

    #[test]
    fn md2tex_fm_only() {
        assert_eq!(
            md_to_latex("---\ntitle: T\nauthor: A\n---\nbody", "Academic Document", "", false, &parse_options("null")),
            "body",
        );
    }

    #[test]
    fn md2tex_fm_short() {
        assert_eq!(
            md_to_latex("---\ntitle: T\n---", "Academic Document", "", false, &parse_options("null")),
            "",
        );
    }

    #[test]
    fn md2tex_fm_single_line() {
        assert_eq!(
            md_to_latex("---\n---\nbody", "Academic Document", "", false, &parse_options("null")),
            "body",
        );
    }

    #[test]
    fn md2tex_fm_sq() {
        assert_eq!(
            md_to_latex("---\ntitle: 'Q T'\n---\nbody", "Academic Document", "", false, &parse_options("null")),
            "body",
        );
    }

    #[test]
    fn md2tex_fm_unclosed() {
        assert_eq!(
            md_to_latex("---\ntitle: T\nbody", "Academic Document", "", false, &parse_options("null")),
            "\\noindent\\rule{\\textwidth}{0.4pt}\ntitle: T\nbody",
        );
    }

    #[test]
    fn md2tex_full_doc() {
        assert_eq!(
            md_to_latex("---\ntitle: \"My Title\"\nauthor: Jane Doe\ndate: 2024-01-02\n---\n\n# Heading 1\n\nIntro **bold** and *ital* and `code` and $a_b$ and ~~gone~~.\n\n## Section 2\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |\n\n> quoted line\n\n- one\n- two\n\n1. first\n2. second\n\n$$\nx = y + z\n$$\n\n```python\nprint(\"hi\")\n```\n\n---\n\nFinal paragraph with [a link](http://x.y) and ![img](p.png).\n\n### \u{811a}\u{6ce8} (Footnotes)\n\n[^1]: note\n", "Academic Document", "", true, &parse_options("null")),
            "\\documentclass[11pt,a4paper]{article}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\n\\usepackage[backend=biber,style=numeric]{biblatex}\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{My Title}\n\\author{Jane Doe}\n\\date{2024-01-02}\n\n\\begin{document}\n\\maketitle\n\n\n\n\\section{Heading 1}\n\nIntro \\textbf{bold} and \\textit{ital} and \\texttt{code} and $a_b$ and \\sout{gone}.\n\n\n\\subsection{Section 2}\n\n\\begin{table}[htbp]\n\\centering\n\\begin{tabular}{ll}\n\\toprule\nA & B \\\\\n\\midrule\n1 & 2 \\\\\n3 & 4 \\\\\n\\bottomrule\n\\end{tabular}\n\\end{table}\n\n\\begin{quote}\nquoted line\n\\end{quote}\n\n\\begin{itemize}\n  \\item one\n  \\item two\n\\end{itemize}\n\n\\begin{enumerate}\n  \\item first\n  \\item second\n\\end{enumerate}\n\n\\begin{equation*}\nx = y + z\n\\end{equation*}\n\n\\begin{lstlisting}[language=python]\nprint(\"hi\")\n\\end{lstlisting}\n\n\\noindent\\rule{\\textwidth}{0.4pt}\n\nFinal paragraph with \\href{http://x.y}{a link} and \\begin{figure}[htbp]\\centering\\includegraphics[max width=\\linewidth]{p.png}\\caption{img}\\end{figure}.\n\n\n\\subsubsection{\u{811a}\u{6ce8} (Footnotes)}\n\n[^1]: note\n\n\\end{document}\n",
        );
    }

    #[test]
    fn md2tex_heading1_7() {
        assert_eq!(
            md_to_latex("#one\n# two", "Academic Document", "", false, &parse_options("null")),
            "#one\n\n\\section{two}",
        );
    }

    #[test]
    fn md2tex_heading6() {
        assert_eq!(
            md_to_latex("###### six\n####### seven", "Academic Document", "", false, &parse_options("null")),
            "\n\\textbf{six}\n####### seven",
        );
    }

    #[test]
    fn md2tex_hr() {
        assert_eq!(
            md_to_latex("***\n---\n___\n----", "Academic Document", "", false, &parse_options("null")),
            "\\noindent\\rule{\\textwidth}{0.4pt}\n\\noindent\\rule{\\textwidth}{0.4pt}\n\\noindent\\rule{\\textwidth}{0.4pt}\n\\noindent\\rule{\\textwidth}{0.4pt}",
        );
    }

    #[test]
    fn md2tex_image() {
        assert_eq!(
            md_to_latex("![alt](img.png)", "Academic Document", "", false, &parse_options("null")),
            "\\begin{figure}[htbp]\\centering\\includegraphics[max width=\\linewidth]{img.png}\\caption{alt}\\end{figure}",
        );
    }

    #[test]
    fn md2tex_inline_bold_ital() {
        assert_eq!(
            md_to_latex("***tri*** and __b__ and _i_", "Academic Document", "", false, &parse_options("null")),
            "\\textbf{\\textit{tri}} and \\textbf{b} and \\textit{i}",
        );
    }

    #[test]
    fn md2tex_inline_math_protect() {
        assert_eq!(
            md_to_latex("a $b_c$ d `x_y` e", "Academic Document", "", false, &parse_options("null")),
            "a $b_c$ d \\texttt{x\\_y} e",
        );
    }

    #[test]
    fn md2tex_link() {
        assert_eq!(
            md_to_latex("[t](u)", "Academic Document", "", false, &parse_options("null")),
            "\\href{u}{t}",
        );
    }

    #[test]
    fn md2tex_lists() {
        assert_eq!(
            md_to_latex("* a\n+ b\n- c", "Academic Document", "", false, &parse_options("null")),
            "\\begin{itemize}\n  \\item a\n  \\item b\n  \\item c\n\\end{itemize}",
        );
    }

    #[test]
    fn md2tex_math_dollars() {
        assert_eq!(
            md_to_latex("$$\n$$$\n$$", "Academic Document", "", false, &parse_options("null")),
            "\\begin{equation*}\n\n\\end{equation*}",
        );
    }

    #[test]
    fn md2tex_math_multiline() {
        assert_eq!(
            md_to_latex("$$\n\\begin{aligned}\na &= b\n\\end{aligned}\n$$", "Academic Document", "", false, &parse_options("null")),
            "\\begin{equation*}\n\\begin{aligned}\na &= b\n\\end{aligned}\n\\end{equation*}",
        );
    }

    #[test]
    fn md2tex_math_oneline() {
        assert_eq!(
            md_to_latex("$$ x $$", "Academic Document", "", false, &parse_options("null")),
            "\\begin{equation*}\nx\n\\end{equation*}",
        );
    }

    #[test]
    fn md2tex_nested_link_bold() {
        assert_eq!(
            md_to_latex("**[t](u)**", "Academic Document", "", false, &parse_options("null")),
            "\\textbf{\\href{u}{t}}",
        );
    }

    #[test]
    fn md2tex_nonstandalone() {
        assert_eq!(
            md_to_latex("# H\ntext", "Academic Document", "", false, &parse_options("null")),
            "\n\\section{H}\ntext",
        );
    }

    #[test]
    fn md2tex_ol() {
        assert_eq!(
            md_to_latex("1. a\n2. b\n3. c", "Academic Document", "", false, &parse_options("null")),
            "\\begin{enumerate}\n  \\item a\n  \\item b\n  \\item c\n\\end{enumerate}",
        );
    }

    #[test]
    fn md2tex_only_ws() {
        assert_eq!(
            md_to_latex("   ", "Academic Document", "", true, &parse_options("null")),
            "\\documentclass[11pt,a4paper]{article}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\n\\usepackage[backend=biber,style=numeric]{biblatex}\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{Academic Document}\n\\author{}\n\\date{\\today}\n\n\\begin{document}\n\\maketitle\n\n\n\n\\end{document}\n",
        );
    }

    #[test]
    fn md2tex_opts_beamer() {
        assert_eq!(
            md_to_latex("body", "Academic Document", "", true, &parse_options("{\"tex\": {\"useCtex\": false, \"docClass\": \"beamer\"}}")),
            "\\documentclass[11pt,a4paper]{beamer}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\n\\usepackage[backend=biber,style=numeric]{biblatex}\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{Academic Document}\n\\author{}\n\\date{\\today}\n\n\\begin{document}\n\\maketitle\n\nbody\n\n\\end{document}\n",
        );
    }

    #[test]
    fn md2tex_opts_bib_natbib() {
        assert_eq!(
            md_to_latex("body", "Academic Document", "", true, &parse_options("{\"tex\": {\"bibEngine\": \"natbib\"}}")),
            "\\documentclass[11pt,a4paper]{article}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\\usepackage{ctex}\n\\usepackage{natbib}\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{Academic Document}\n\\author{}\n\\date{\\today}\n\n\\begin{document}\n\\maketitle\n\nbody\n\n\\end{document}\n",
        );
    }

    #[test]
    fn md2tex_opts_bib_none() {
        assert_eq!(
            md_to_latex("body", "Academic Document", "", true, &parse_options("{\"tex\": {\"bibEngine\": \"none\"}}")),
            "\\documentclass[11pt,a4paper]{article}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\\usepackage{ctex}\n\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{Academic Document}\n\\author{}\n\\date{\\today}\n\n\\begin{document}\n\\maketitle\n\nbody\n\n\\end{document}\n",
        );
    }

    #[test]
    fn md2tex_opts_ctex_false() {
        assert_eq!(
            md_to_latex("body", "Academic Document", "", true, &parse_options("{\"tex\": {\"useCtex\": false}}")),
            "\\documentclass[11pt,a4paper]{article}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\n\\usepackage[backend=biber,style=numeric]{biblatex}\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{Academic Document}\n\\author{}\n\\date{\\today}\n\n\\begin{document}\n\\maketitle\n\nbody\n\n\\end{document}\n",
        );
    }

    #[test]
    fn md2tex_opts_empty() {
        assert_eq!(
            md_to_latex("body", "Academic Document", "", true, &parse_options("{}")),
            "\\documentclass[11pt,a4paper]{article}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\n\\usepackage[backend=biber,style=numeric]{biblatex}\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{Academic Document}\n\\author{}\n\\date{\\today}\n\n\\begin{document}\n\\maketitle\n\nbody\n\n\\end{document}\n",
        );
    }

    #[test]
    fn md2tex_opts_latex_key() {
        assert_eq!(
            md_to_latex("body", "Academic Document", "", true, &parse_options("{\"latex\": {\"docClass\": \"ctexart\"}}")),
            "\\documentclass[11pt,a4paper]{ctexart}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\n\\usepackage[backend=biber,style=numeric]{biblatex}\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{Academic Document}\n\\author{}\n\\date{\\today}\n\n\\begin{document}\n\\maketitle\n\nbody\n\n\\end{document}\n",
        );
    }

    #[test]
    fn md2tex_opts_margins() {
        assert_eq!(
            md_to_latex("body", "Academic Document", "", true, &parse_options("{\"tex\": {\"margin\": \"3cm\", \"fontSize\": \"12pt\", \"paperSize\": \"letter\"}}")),
            "\\documentclass[12pt,letter]{article}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=3cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\\usepackage{ctex}\n\\usepackage[backend=biber,style=numeric]{biblatex}\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{Academic Document}\n\\author{}\n\\date{\\today}\n\n\\begin{document}\n\\maketitle\n\nbody\n\n\\end{document}\n",
        );
    }

    #[test]
    fn md2tex_opts_meta() {
        assert_eq!(
            md_to_latex("body", "Academic Document", "", true, &parse_options("{\"meta\": {\"title\": \"MetaT\", \"author\": \"MetaA\"}}")),
            "\\documentclass[11pt,a4paper]{article}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\n\\usepackage[backend=biber,style=numeric]{biblatex}\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{MetaT}\n\\author{MetaA}\n\\date{\\today}\n\n\\begin{document}\n\\maketitle\n\nbody\n\n\\end{document}\n",
        );
    }

    #[test]
    fn md2tex_opts_title_none() {
        assert_eq!(
            md_to_latex("body", "Academic Document", "", true, &parse_options("{\"tex\": {\"title\": \"\"}}")),
            "\\documentclass[11pt,a4paper]{article}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\\usepackage{ctex}\n\\usepackage[backend=biber,style=numeric]{biblatex}\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{Academic Document}\n\\author{}\n\\date{\\today}\n\n\\begin{document}\n\\maketitle\n\nbody\n\n\\end{document}\n",
        );
    }

    #[test]
    fn md2tex_opts_title_null() {
        assert_eq!(
            md_to_latex("body", "Academic Document", "", true, &parse_options("{\"tex\": {\"title\": null}}")),
            "\\documentclass[11pt,a4paper]{article}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\\usepackage{ctex}\n\\usepackage[backend=biber,style=numeric]{biblatex}\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{Academic Document}\n\\author{}\n\\date{\\today}\n\n\\begin{document}\n\\maketitle\n\nbody\n\n\\end{document}\n",
        );
    }

    #[test]
    fn md2tex_opts_use_ctex_null() {
        assert_eq!(
            md_to_latex("body", "Academic Document", "", true, &parse_options("{\"tex\": {\"useCtex\": null}}")),
            "\\documentclass[11pt,a4paper]{article}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\n\\usepackage[backend=biber,style=numeric]{biblatex}\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{Academic Document}\n\\author{}\n\\date{\\today}\n\n\\begin{document}\n\\maketitle\n\nbody\n\n\\end{document}\n",
        );
    }

    #[test]
    fn md2tex_quote() {
        assert_eq!(
            md_to_latex("> hello *x*", "Academic Document", "", false, &parse_options("null")),
            "\\begin{quote}\nhello \\textit{x}\n\\end{quote}",
        );
    }

    #[test]
    fn md2tex_specials() {
        assert_eq!(
            md_to_latex("100% done & cost $5 # a_b ^ ~", "Academic Document", "", false, &parse_options("null")),
            "100% done & cost $5 # a_b ^ ~",
        );
    }

    #[test]
    fn md2tex_table_align() {
        assert_eq!(
            md_to_latex("| a | b |\n|:----|---:|\n| c | d |", "Academic Document", "", false, &parse_options("null")),
            "\\begin{table}[htbp]\n\\centering\n\\begin{tabular}{ll}\n\\toprule\na & b \\\\\n\\midrule\nc & d \\\\\n\\bottomrule\n\\end{tabular}\n\\end{table}",
        );
    }

    #[test]
    fn md2tex_table_uneven() {
        assert_eq!(
            md_to_latex("| a | b |\n| --- | --- |\n| c | d | e |", "Academic Document", "", false, &parse_options("null")),
            "\\begin{table}[htbp]\n\\centering\n\\begin{tabular}{lll}\n\\toprule\na & b &  \\\\\n\\midrule\nc & d & e \\\\\n\\bottomrule\n\\end{tabular}\n\\end{table}",
        );
    }

    #[test]
    fn md2tex_title_args() {
        assert_eq!(
            md_to_latex("body", "Given", "Auth", true, &parse_options("{\"tex\": {\"title\": \"Win\"}}")),
            "\\documentclass[11pt,a4paper]{article}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\\usepackage{ctex}\n\\usepackage[backend=biber,style=numeric]{biblatex}\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{Win}\n\\author{Auth}\n\\date{\\today}\n\n\\begin{document}\n\\maketitle\n\nbody\n\n\\end{document}\n",
        );
    }

    #[test]
    fn md2tex_trailing_nl() {
        assert_eq!(
            md_to_latex("a\n", "Academic Document", "", false, &parse_options("null")),
            "a",
        );
    }

    #[test]
    fn md2tex_underscore() {
        assert_eq!(
            md_to_latex("a_b_c and _em_", "Academic Document", "", false, &parse_options("null")),
            "a_b_c and \\textit{em}",
        );
    }

    #[test]
    fn roundtrip_0() {
        let tex = md_to_latex("# T\n\nbody **b** and $x_y$\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n", "Academic Document", "", false, &OptVal::Null);
        assert_eq!(tex, "\n\\section{T}\n\nbody \\textbf{b} and $x_y$\n\n\\begin{table}[htbp]\n\\centering\n\\begin{tabular}{ll}\n\\toprule\na & b \\\\\n\\midrule\n1 & 2 \\\\\n\\bottomrule\n\\end{tabular}\n\\end{table}");
        assert_eq!(latex_to_md(&tex, ""), "# T\n\nbody **b** and $x_y$\n\n| a | b |\n| --- | --- |\n| 1 | 2 |");
    }

    #[test]
    fn roundtrip_1() {
        let tex = md_to_latex("## S2\n\n- one\n- two\n\n> quote\n\n$$\na = b\n$$\n", "Academic Document", "", false, &OptVal::Null);
        assert_eq!(tex, "\n\\subsection{S2}\n\n\\begin{itemize}\n  \\item one\n  \\item two\n\\end{itemize}\n\n\\begin{quote}\nquote\n\\end{quote}\n\n\\begin{equation*}\na = b\n\\end{equation*}");
        assert_eq!(latex_to_md(&tex, ""), "## S2\n\n- one\n- two\n\n> quote\n\n$$\na = b\n$$");
    }

    #[test]
    fn roundtrip_2() {
        let tex = md_to_latex("# \u{4e2d}\u{6587}\n\n\u{7532} **\u{4e59}** $\\alpha$ \u{4e19}\n\n1. \u{4e00}\n2. \u{4e8c}\n", "Academic Document", "", false, &OptVal::Null);
        assert_eq!(tex, "\n\\section{\u{4e2d}\u{6587}}\n\n\u{7532} \\textbf{\u{4e59}} $\\alpha$ \u{4e19}\n\n\\begin{enumerate}\n  \\item \u{4e00}\n  \\item \u{4e8c}\n\\end{enumerate}");
        assert_eq!(latex_to_md(&tex, ""), "# \u{4e2d}\u{6587}\n\n\u{7532} **\u{4e59}** $\\alpha$ \u{4e19}\n\n1. \u{4e00}\n2. \u{4e8c}");
    }

    #[test]
    fn roundtrip_3() {
        let tex = md_to_latex("---\ntitle: \"R\"\nauthor: A\n---\n\n# H\n\ntext `c`\n", "Academic Document", "", false, &OptVal::Null);
        assert_eq!(tex, "\n\n\\section{H}\n\ntext \\texttt{c}");
        assert_eq!(latex_to_md(&tex, ""), "# H\n\ntext `c`");
    }

    #[test]
    fn macro_builtins() {
        let ex = MacroExpander::new();
        assert_eq!(ex.expand("\\bs{\\alpha} \\in \\R, \\degree C, \\i + \\e"), "\\mathbf{\\alpha} \\in \\mathbb{R}, ^\\circ C, \\mathrm{i} + \\mathrm{e}");
    }

    #[test]
    fn macro_preamble() {
        let mut ex = MacroExpander::new();
        assert_eq!(ex.parse_preamble_macros("\\newcommand{\\f}[2]{#1^#2}\n\\renewcommand\\g{\\beta}\n\\def\\h#1#2#3{(#1,#2,#3)}\n\\DeclareMathOperator{\\lcm}{lcm}\nbody \\f{a}{b} \\g \\h{1}{2}{3} \\lcm(2,3)\n"), "\n\n\n\nbody \\f{a}{b} \\g \\h{1}{2}{3} \\lcm(2,3)\n");
        assert_eq!(ex.expand("body \\f{a}{b} \\g \\h{1}{2}{3} \\lcm(2,3)"), "body a^b \\beta (1,2,3) \\operatorname{lcm}(2,3)");
    }

    #[test]
    fn macro_nested() {
        let mut ex = MacroExpander::new();
        assert_eq!(ex.parse_preamble_macros("\\newcommand{\\B}{x}\n\\newcommand{\\A}{\\B plus \\B}\n"), "\n\n");
        assert_eq!(ex.expand("\\A and \\A"), "x plus x and x plus x");
    }

    #[test]
    fn macro_recursive_default() {
        let mut ex = MacroExpander::new();
        assert_eq!(ex.parse_preamble_macros("\\newcommand{\\L}{\\L!}\n"), "\n");
        assert_eq!(ex.expand("\\L"), "\\L!!!!!");
    }

    #[test]
    fn macro_recursive_depth1() {
        let mut ex = MacroExpander::new();
        assert_eq!(ex.parse_preamble_macros("\\newcommand{\\L}{\\L!}\n"), "\n");
        assert_eq!(chars_to_string(&ex.expand_c(&to_chars("\\L"), 1)), "\\L!");
    }

    #[test]
    fn macro_recursive_depth0() {
        let mut ex = MacroExpander::new();
        assert_eq!(ex.parse_preamble_macros("\\newcommand{\\L}{\\L!}\n"), "\n");
        assert_eq!(chars_to_string(&ex.expand_c(&to_chars("\\L"), 0)), "\\L");
    }

    #[test]
    fn macro_undefined() {
        let ex = MacroExpander::new();
        assert_eq!(ex.expand("\\undefinedthing{x} \\mathbf{y}"), "\\undefinedthing{x} \\mathbf{y}");
    }

    #[test]
    fn macro_empty_macros_input() {
        let ex = MacroExpander::new();
        assert_eq!(ex.expand("\\R"), "\\mathbb{R}");
    }

    #[test]
    fn macro_braceless_newcommand() {
        let mut ex = MacroExpander::new();
        assert_eq!(ex.parse_preamble_macros("\\renewcommand\\Z{Z}\n"), "\n");
        assert_eq!(ex.expand("\\Z"), "Z");
    }

    #[test]
    fn macro_bad_numargs() {
        let mut ex = MacroExpander::new();
        assert_eq!(ex.parse_preamble_macros("\\newcommand{\\y}[2]{q}\n"), "\n");
        assert_eq!(ex.expand("\\y{1}"), "q");
    }

    #[test]
    fn macro_param_edge() {
        let mut ex = MacroExpander::new();
        assert_eq!(ex.parse_preamble_macros("\\newcommand{\\p}[2]{#1|#2}\n"), "\n");
        assert_eq!(ex.expand("\\p a b \\p{ x}{y} \\p{z} \\p{q}"), "a|b  x|y z|q|");
    }

    #[test]
    fn macro_arg_missing_eof() {
        let mut ex = MacroExpander::new();
        assert_eq!(ex.parse_preamble_macros("\\newcommand{\\q}[2]{[#1|#2]}\n"), "\n");
        assert_eq!(ex.expand("\\q "), "[|]");
    }

    #[test]
    fn macro_comment_stripped() {
        let mut ex = MacroExpander::new();
        assert_eq!(ex.parse_preamble_macros("% \\newcommand{\\c}{x}\n"), "% \n");
        assert_eq!(ex.expand("\\c and \\R"), "x and \\mathbb{R}");
    }

    #[test]
    fn extract_balanced_0() {
        let (v, p) = extract_balanced(&to_chars("ab{c}de"), 2, '{', '}');
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), Some("c"));
        assert_eq!(p, 5);
    }

    #[test]
    fn extract_balanced_1() {
        let (v, p) = extract_balanced(&to_chars("ab"), 5, '{', '}');
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), None);
        assert_eq!(p, 5);
    }

    #[test]
    fn extract_balanced_2() {
        let (v, p) = extract_balanced(&to_chars("ab"), 2, '{', '}');
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), None);
        assert_eq!(p, 2);
    }

    #[test]
    fn extract_balanced_3() {
        let (v, p) = extract_balanced(&to_chars("{a\\\\{b}c}"), 0, '{', '}');
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), Some("a\\\\{b}c"));
        assert_eq!(p, 9);
    }

    #[test]
    fn extract_balanced_4() {
        let (v, p) = extract_balanced(&to_chars("  {a}"), 0, '{', '}');
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), Some("a"));
        assert_eq!(p, 5);
    }

    #[test]
    fn extract_balanced_5() {
        let (v, p) = extract_balanced(&to_chars("{a"), 0, '{', '}');
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), Some("a"));
        assert_eq!(p, 2);
    }

    #[test]
    fn extract_balanced_6() {
        let (v, p) = extract_balanced(&to_chars("[opt]x"), 0, '[', ']');
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), Some("opt"));
        assert_eq!(p, 5);
    }

    #[test]
    fn extract_balanced_7() {
        let (v, p) = extract_balanced(&to_chars("  \\x"), 0, '{', '}');
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), None);
        assert_eq!(p, 0);
    }

    #[test]
    fn extract_opt_arg_0() {
        let (v, p) = extract_opt_arg(&to_chars("{a}"), 0);
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), None);
        assert_eq!(p, 0);
    }

    #[test]
    fn extract_mand_arg_0() {
        let (v, p) = extract_mand_arg(&to_chars("{a}"), 0);
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), Some("a"));
        assert_eq!(p, 3);
    }

    #[test]
    fn extract_opt_arg_1() {
        let (v, p) = extract_opt_arg(&to_chars("  {a}"), 0);
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), None);
        assert_eq!(p, 0);
    }

    #[test]
    fn extract_mand_arg_1() {
        let (v, p) = extract_mand_arg(&to_chars("  {a}"), 0);
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), Some("a"));
        assert_eq!(p, 5);
    }

    #[test]
    fn extract_opt_arg_2() {
        let (v, p) = extract_opt_arg(&to_chars("x"), 0);
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), None);
        assert_eq!(p, 0);
    }

    #[test]
    fn extract_mand_arg_2() {
        let (v, p) = extract_mand_arg(&to_chars("x"), 0);
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), None);
        assert_eq!(p, 0);
    }

    #[test]
    fn extract_opt_arg_3() {
        let (v, p) = extract_opt_arg(&to_chars("[o]{a}"), 0);
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), Some("o"));
        assert_eq!(p, 3);
    }

    #[test]
    fn extract_mand_arg_3() {
        let (v, p) = extract_mand_arg(&to_chars("[o]{a}"), 0);
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), None);
        assert_eq!(p, 0);
    }

    #[test]
    fn extract_opt_arg_4() {
        let (v, p) = extract_opt_arg(&to_chars("  [o]"), 1);
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), Some("o"));
        assert_eq!(p, 5);
    }

    #[test]
    fn extract_mand_arg_4() {
        let (v, p) = extract_mand_arg(&to_chars("  [o]"), 1);
        assert_eq!(v.map(|x| chars_to_string(&x)).as_deref(), None);
        assert_eq!(p, 1);
    }

    #[test]
    fn unit_escape_plain() {
        assert_eq!(
            escape_latex_plain_text("a&b%c$d#e_f{g}h~i^j\\k"),
            "a\\&b\\%c\\$d\\#e\\_f\\{g\\}h\\textasciitilde{}i\\textasciicircum{}j\\k",
        );
    }

    #[test]
    fn unit_inline_md() {
        assert_eq!(
            convert_inline_md_to_latex("**b** and `code` and $a_b$ and ~~s~~ and _i_ and __u__"),
            "\\textbf{b} and \\texttt{code} and $a_b$ and \\sout{s} and \\textit{i} and \\textbf{u}",
        );
    }

    #[test]
    fn unit_label_identity() {
        assert_eq!(
            latex_label("A \\and B"),
            "A \\and B",
        );
    }

    #[test]
    fn unit_booktabs() {
        assert_eq!(
            render_latex_booktabs_table(&[vec!["a".to_string(), "b".to_string()], vec!["c".to_string()]]).join("\n"),
            "\\begin{table}[htbp]\n\\centering\n\\begin{tabular}{ll}\n\\toprule\na & b \\\\\n\\midrule\nc &  \\\\\n\\bottomrule\n\\end{tabular}\n\\end{table}",
        );
    }

    #[test]
    fn unit_template_default() {
        assert_eq!(
            build_latex_template(&OptVal::Null),
            "\\documentclass[11pt,a4paper]{article}\n\n% --- \u{6838}\u{5fc3}\u{6570}\u{5b66}\u{4e0e}\u{5b66}\u{672f}\u{5b8f}\u{5305} ---\n\\usepackage[utf8]{inputenc}\n\\usepackage[margin=2.5cm]{geometry}\n\\usepackage{amsmath,amssymb,amsfonts,amsthm,mathtools}\n\\usepackage{booktabs}\n\\usepackage{tabularx}\n\\usepackage{multirow}\n\\usepackage{graphicx}\n\\usepackage{hyperref}\n\\usepackage{listings}\n\\usepackage{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{microtype}\n\n\\usepackage[backend=biber,style=numeric]{biblatex}\n\n% --- \u{8d85}\u{94fe}\u{63a5}\u{4e0e}\u{4e3b}\u{9898}\u{8272}\u{5f69} ---\n\\hypersetup{\n    colorlinks=true,\n    linkcolor=blue!70!black,\n    citecolor=blue!70!black,\n    urlcolor=blue!70!black\n}\n\n% --- \u{4ee3}\u{7801}\u{5757}\u{6837}\u{5f0f} ---\n\\lstset{\n    basicstyle=\\ttfamily\\small,\n    breaklines=true,\n    frame=single,\n    backgroundcolor=\\color{gray!8},\n    keywordstyle=\\color{blue!80!black},\n    commentstyle=\\color{green!50!black},\n    stringstyle=\\color{red!70!black},\n    showstringspaces=false\n}\n\n% --- \u{5f15}\u{7528}\u{5757}\u{4e0e}\u{63d0}\u{793a}\u{6846} ---\n\\tcolorboxenvironment{quote}{\n    colback=gray!5,\n    colframe=gray!40,\n    arc=2mm,\n    left=3mm,\n    right=3mm,\n    top=2mm,\n    bottom=2mm\n}\n\n\\title{__TITLE__}\n\\author{__AUTHOR__}\n\\date{__DATE__}\n\n\\begin{document}\n\\maketitle\n\n__CONTENT__\n\n\\end{document}\n",
        );
    }

    #[test]
    fn unit_splitlines() {
        assert_eq!(
            py_splitlines("a\r\nb\nc\rd\u{b}e\u{c}f\u{1c}\u{e2}\u{82}\u{a8}g"),
            vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string(), "e".to_string(), "f".to_string(), "\u{e2}\u{82}\u{a8}g".to_string()],
        );
    }

    #[test]
    fn unit_strip() {
        assert_eq!(
            py_strip(" \u{b}\u{1c}\u{85}\u{2028}x\u{2029}\u{1f} \t"),
            "x",
        );
    }

    #[test]
    fn unit_slice_oob() {
        assert_eq!(
            chars_to_string(&py_slice(&to_chars("\u{4e2d}\u{6587}a"), 2, 99)),
            "a",
        );
    }

    #[test]
    fn unit_slice_neg() {
        assert_eq!(
            chars_to_string(&py_slice(&to_chars("abc"), 0, usize::MAX)),
            "abc",
        );
    }

    #[test]
    fn tex2md_input_recursion() {
        let dir = std::env::temp_dir().join("readmd_texmd_inputs_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("main.tex"), "\\begin{document}\n\\input{child}\ntop level\n\\input{sub/deep.tex}\n\\input{missing}\n\\input{latin}\n\\end{document}\n".as_bytes()).unwrap();
        std::fs::write(dir.join("child.tex"), "\\begin{document}\nchild body \\textbf{x}\n% \\newcommand{\\nope}{y}\n\\end{document}\n".as_bytes()).unwrap();
        std::fs::write(dir.join("sub").join("deep.tex"), "\\input{child} deep body \\item z\n".as_bytes()).unwrap();
        std::fs::write(dir.join("latin.tex"), &[0x63u8, 0x61, 0x66, 0xe9, 0x20, 0x6c, 0x61, 0x74,
            0x69, 0x6e, 0x2d, 0x31, 0x20, 0x74, 0x61, 0x69, 0x6c, 0x20, 0x25, 0x20, 0x63, 0x6f,
            0x6d, 0x6d, 0x65, 0x6e, 0x74, 0x0a][..]).unwrap();
        let base = dir.to_string_lossy().to_string();
        let main_src = std::fs::read_to_string(dir.join("main.tex")).unwrap();
        assert_eq!(latex_to_md(&main_src, &base), "child body **x**\n\ntop level\n\nchild body **x**\n\n deep body - z\n\ncaf\u{e9} latin-1 tail");
        let _ = std::fs::remove_dir_all(&dir);
    }

}

// <<< END GENERATED PARITY TESTS

#[cfg(test)]
mod tests_engine {
    use super::*;

    fn c(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn literal_and_classes() {
        let r = Re::new(r"a+b", Fl::NONE);
        assert!(r.searched(&c("xaaab")));
        assert!(!r.searched(&c("xbb")));
        let r = Re::new(r"[^a-z]+", Fl::NONE);
        let s = c("ABz");
        assert_eq!(r.match_(&s).unwrap().end, 2);
        let r = Re::new(r"\s+", Fl::NONE);
        // CPython whitespace includes U+001C..U+001F
        assert_eq!(r.match_(&c("\u{1c}x")).unwrap().end, 1);
        let r = Re::new(r"^\d+$", Fl::NONE);
        assert!(r.is_match(&c("123")));
        assert!(!r.is_match(&c("12a")));
    }

    #[test]
    fn dot_does_not_cross_newline_without_dotall() {
        let s1 = c("a\nb");
        let lazy = Re::new(r"a(.*?)b", Fl::NONE);
        assert!(!lazy.searched(&s1));
        let dotall = Re::new(r"a(.*?)b", Fl::DOTALL);
        let m = dotall.match_(&s1).unwrap();
        assert_eq!(m.gs(&s1, 1).unwrap(), "\n");
    }

    #[test]
    fn anchors_follow_cpython_dollar() {
        let r = Re::new(r"a$", Fl::NONE);
        assert!(r.searched(&c("ba")));
        assert!(r.searched(&c("ba\n")));
        assert!(!r.searched(&c("ba\r\n")));
    }

    #[test]
    fn greedy_vs_lazy() {
        let s = c("<a><b>");
        let g = Re::new(r"<(.*)>", Fl::NONE);
        assert_eq!(g.match_(&s).unwrap().gs(&s, 1).unwrap(), "a><b");
        let l = Re::new(r"<(.*?)>", Fl::NONE);
        assert_eq!(l.match_(&s).unwrap().gs(&s, 1).unwrap(), "a");
    }

    #[test]
    fn backreference_like_verb() {
        let s = c(r"\verb|a+b| tail");
        let r = Re::new(r"\\verb(.)(.*?)\1", Fl::NONE);
        assert_eq!(chars_to_string(&r.replace_all(&s, r"`\2`")), "`a+b` tail");
    }

    #[test]
    fn lookbehind_and_ahead() {
        let s = c(r"100\% ok %cut");
        let r = Re::new(r"(?<!\\)%.*$", Fl::NONE);
        assert_eq!(chars_to_string(&r.replace_all(&s, "")), r"100\% ok ");
        let r2 = Re::new(r"\\item(?:\s+|(?=[\\$]))", Fl::NONE);
        let parts = r2.split(&c("a \\item b \\item$c"));
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].as_deref(), Some("a "));
        assert_eq!(parts[2].as_deref(), Some("$c"));
    }

    #[test]
    fn counted_repeat() {
        let r = Re::new(r"^(\*{3,}|-{3,}|_{3,})$", Fl::NONE);
        assert!(r.is_match(&c("***")));
        assert!(r.is_match(&c("-----")));
        assert!(!r.is_match(&c("**")));
        let r2 = Re::new(r"(?<!\w)_(.*?)_{1}(?!\w)", Fl::NONE);
        let s = c("a_b_c and _em_");
        assert_eq!(
            chars_to_string(&r2.replace_all(&s, r"\\textit{\1}")),
            "a_b_c and \\textit{em}"
        );
    }

    #[test]
    fn optional_group_repetition() {
        let r =
            Re::new(r"\\(?:cite|citep)(?:\[.*?\])*\{([^}]+)\}", Fl::NONE);
        let s = c(r"\cite[p.1]{a, b}\citep{c}");
        let hits: Vec<String> = r.iter(&s).map(|m| m.gs_or_empty(&s, 1)).collect();
        assert_eq!(hits, vec!["a, b".to_string(), "c".to_string()]);
    }

    #[test]
    fn split_interleaves_groups() {
        let r = Re::new(r"\\item(?:\[(.*?)\])?(?:\s+|(?=[\\$]))", Fl::NONE);
        let parts = r.split(&c("body\\item one\\item[Note] two"));
        assert_eq!(parts[0].as_deref(), Some("body"));
        assert_eq!(parts[1], None);
        assert_eq!(parts[2].as_deref(), Some("one"));
        assert_eq!(parts[3].as_deref(), Some("Note"));
        assert_eq!(parts[4].as_deref(), Some("two"));
    }

    #[test]
    fn ignorecase_class() {
        let r = Re::new(r"language\s*=\s*([a-zA-Z0-9_\+#]+)", Fl::IC);
        let s = c("LANGUAGE = Python");
        let m = r.search(&s, 0).unwrap();
        assert_eq!(m.gs_or_empty(&s, 1), "Python");
    }

    #[test]
    fn unicode_indices_are_code_points() {
        let s = c("中文\\textbf{加粗}");
        let r = Re::new(r"\\textbf\{([^}]*)\}", Fl::NONE);
        let m = r.search(&s, 0).unwrap();
        assert_eq!(m.gs(&s, 1).unwrap(), "加粗");
        assert_eq!(m.start, 2);
    }
}
