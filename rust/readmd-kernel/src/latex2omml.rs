// -*- coding: utf-8 -*-
//! LaTeX -> OMML (Office Math Markup Language) 编译器。
//!
//! 将 LaTeX 数学公式字符串直接解析编译为 Microsoft Word 原生的 OMML XML 节点，
//! 支持在 python-docx 生成的 .docx 文档中直接插入可编辑的矢量微软公式对象。
//!
//! 支持语法：
//! 1. 基础结构：数字、变量、四则运算、正负号、等号、空格 (\quad, \;, \,)
//! 2. 分数与二项式：\frac{a}{b}, \dfrac{a}{b}, \binom{n}{k}
//! 3. 上下标与组合：x^2, x_i, x_i^2, {x_i}^2
//! 4. 根式与开方：\sqrt{x}, \sqrt[n]{x}
//! 5. 大型运算符：\sum, \int, \iint, \iiint, \oint, \prod, \bigcup, \bigcap, \lim
//! 6. 智能定界符：\left( ... \right), \left[ ... \right], \left\{ ... \right\}, \left| ... \right|, \left\| ... \right\|
//! 7. 矩阵与行列式：\begin{matrix}, \begin{pmatrix}, \begin{bmatrix}, \begin{vmatrix}, \begin{aligned}
//! 8. 常见数学函数：\sin, \cos, \tan, \cot, \sec, \csc, \ln, \log, \exp, \max, \min, \det, \dim
//! 9. 希腊字母与数学符号：\alpha, \beta, \gamma, \theta, \pi, \pm, \times, \div, \leq, \geq, \neq, \approx, \infty, \partial, \nabla, \to 等 100+ 符号
//! 10. 重音与修饰符：\vec, \hat, \bar, \dot, \ddot, \tilde, \overline, \boxed

use lazy_static::lazy_static;
use std::collections::{HashMap, VecDeque};

const _M_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";
const _W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

lazy_static! {
    /// 希腊字母与常用数学符号映射表
    static ref MATH_SYMBOLS: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        // 小写希腊字母
        m.insert(r"\alpha", "α");
        m.insert(r"\beta", "β");
        m.insert(r"\gamma", "γ");
        m.insert(r"\delta", "δ");
        m.insert(r"\epsilon", "ϵ");
        m.insert(r"\varepsilon", "ε");
        m.insert(r"\zeta", "ζ");
        m.insert(r"\eta", "η");
        m.insert(r"\theta", "θ");
        m.insert(r"\vartheta", "ϑ");
        m.insert(r"\iota", "ι");
        m.insert(r"\kappa", "κ");
        m.insert(r"\lambda", "λ");
        m.insert(r"\mu", "μ");
        m.insert(r"\nu", "ν");
        m.insert(r"\xi", "ξ");
        m.insert(r"\pi", "π");
        m.insert(r"\varpi", "ϖ");
        m.insert(r"\rho", "ρ");
        m.insert(r"\varrho", "ϱ");
        m.insert(r"\sigma", "σ");
        m.insert(r"\varsigma", "ς");
        m.insert(r"\tau", "τ");
        m.insert(r"\upsilon", "υ");
        m.insert(r"\phi", "ϕ");
        m.insert(r"\varphi", "φ");
        m.insert(r"\chi", "χ");
        m.insert(r"\psi", "ψ");
        m.insert(r"\omega", "ω");
        // 大写希腊字母
        m.insert(r"\Gamma", "Γ");
        m.insert(r"\Delta", "Δ");
        m.insert(r"\Theta", "Θ");
        m.insert(r"\Lambda", "Λ");
        m.insert(r"\Xi", "Ξ");
        m.insert(r"\Pi", "Π");
        m.insert(r"\Sigma", "Σ");
        m.insert(r"\Upsilon", "Υ");
        m.insert(r"\Phi", "Φ");
        m.insert(r"\Psi", "Ψ");
        m.insert(r"\Omega", "Ω");
        // 运算符与关系符
        m.insert(r"\pm", "±");
        m.insert(r"\mp", "∓");
        m.insert(r"\times", "×");
        m.insert(r"\div", "÷");
        m.insert(r"\cdot", "·");
        m.insert(r"\ast", "∗");
        m.insert(r"\star", "⋆");
        m.insert(r"\circ", "∘");
        m.insert(r"\bullet", "∙");
        m.insert(r"\otimes", "⊗");
        m.insert(r"\oplus", "⊕");
        m.insert(r"\odot", "⊙");
        m.insert(r"\leq", "≤");
        m.insert(r"\le", "≤");
        m.insert(r"\geq", "≥");
        m.insert(r"\ge", "≥");
        m.insert(r"\neq", "≠");
        m.insert(r"\ne", "≠");
        m.insert(r"\approx", "≈");
        m.insert(r"\equiv", "≡");
        m.insert(r"\sim", "∼");
        m.insert(r"\simeq", "≃");
        m.insert(r"\cong", "≅");
        m.insert(r"\propto", "∝");
        m.insert(r"\ll", "≪");
        m.insert(r"\gg", "≫");
        m.insert(r"\prec", "≺");
        m.insert(r"\succ", "≻");
        // 箭头
        m.insert(r"\to", "→");
        m.insert(r"\rightarrow", "→");
        m.insert(r"\leftarrow", "←");
        m.insert(r"\Rightarrow", "⇒");
        m.insert(r"\Leftarrow", "⇐");
        m.insert(r"\leftrightarrow", "↔");
        m.insert(r"\Leftrightarrow", "⇔");
        m.insert(r"\mapsto", "↦");
        m.insert(r"\uparrow", "↑");
        m.insert(r"\downarrow", "↓");
        // 微积分与分析
        m.insert(r"\infty", "∞");
        m.insert(r"\partial", "∂");
        m.insert(r"\nabla", "∇");
        m.insert(r"\prime", "′");
        m.insert(r"\hbar", "ℏ");
        m.insert(r"\ell", "ℓ");
        // 集合与逻辑
        m.insert(r"\in", "∈");
        m.insert(r"\notin", "∉");
        m.insert(r"\subset", "⊂");
        m.insert(r"\subseteq", "⊆");
        m.insert(r"\supset", "⊃");
        m.insert(r"\supseteq", "⊇");
        m.insert(r"\cap", "∩");
        m.insert(r"\cup", "∪");
        m.insert(r"\setminus", "∖");
        m.insert(r"\forall", "∀");
        m.insert(r"\exists", "∃");
        m.insert(r"\neg", "¬");
        m.insert(r"\land", "∧");
        m.insert(r"\lor", "∨");
        m.insert(r"\emptyset", "∅");
        m.insert(r"\varnothing", "∅");
        // 标点与省略号
        m.insert(r"\ldots", "…");
        m.insert(r"\cdots", "⋯");
        m.insert(r"\vdots", "⋮");
        m.insert(r"\ddots", "⋱");
        m.insert(r"\angle", "∠");
        m.insert(r"\perp", "⊥");
        m.insert(r"\parallel", "∥");
        m.insert(r"\langle", "⟨");
        m.insert(r"\rangle", "⟩");
        m.insert(r"\vert", "|");
        m.insert(r"\,", " ");
        m.insert(r"\;", " ");
        m.insert(r"\quad", "  ");
        m.insert(r"\qquad", "    ");
        m.insert(r"\!", "");
        m.insert(r"\%", "%");
        m.insert(r"\_", "_");
        m.insert(r"\&", "&");
        m.insert(r"\#", "#");
        m
    };
}

lazy_static! {
    /// 一元/多元运算符映射
    static ref NARY_OPS: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert(r"\sum", "∑");
        m.insert(r"\prod", "∏");
        m.insert(r"\coprod", "∐");
        m.insert(r"\int", "∫");
        m.insert(r"\iint", "∬");
        m.insert(r"\iiint", "∭");
        m.insert(r"\oint", "∮");
        m.insert(r"\bigcap", "⋂");
        m.insert(r"\bigcup", "⋃");
        m
    };
}

lazy_static! {
    /// 重音符号映射
    static ref ACCENTS: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert(r"\hat", "^");
        m.insert(r"\bar", "¯");
        m.insert(r"\vec", "→");
        m.insert(r"\dot", "˙");
        m.insert(r"\ddot", "¨");
        m.insert(r"\tilde", "~");
        m.insert(r"\check", "ˇ");
        m.insert(r"\acute", "´");
        m.insert(r"\grave", "`");
        m
    };
}

lazy_static! {
    /// 常见数学函数集
    static ref FUNCTIONS: std::collections::HashSet<&'static str> = {
        let mut s = std::collections::HashSet::new();
        s.insert(r"\sin");
        s.insert(r"\cos");
        s.insert(r"\tan");
        s.insert(r"\cot");
        s.insert(r"\sec");
        s.insert(r"\csc");
        s.insert(r"\arcsin");
        s.insert(r"\arccos");
        s.insert(r"\arctan");
        s.insert(r"\sinh");
        s.insert(r"\cosh");
        s.insert(r"\tanh");
        s.insert(r"\ln");
        s.insert(r"\log");
        s.insert(r"\lg");
        s.insert(r"\exp");
        s.insert(r"\det");
        s.insert(r"\dim");
        s.insert(r"\ker");
        s.insert(r"\deg");
        s.insert(r"\gcd");
        s.insert(r"\hom");
        s.insert(r"\inf");
        s.insert(r"\sup");
        s.insert(r"\lim");
        s.insert(r"\max");
        s.insert(r"\min");
        s
    };
}

/// LaTeX 公式分词器
///
/// Owns its (Python-`strip`ed) text: the iterative parser keeps one tokenizer per
/// heap frame, so a borrowed `&'a str` would make `Frame` self-referential.
/// CPython's `__init__` likewise binds `self.text = text.strip()`, which
/// allocates a new `str` whenever anything is trimmed off.
struct LatexTokenizer {
    text: String,
    pos: usize,
    len: usize,
}

impl LatexTokenizer {
    /// Only the `cfg(test)` recursive oracle still borrows its input; the
    /// iterative parser owns every frame's text (see `Frame::new`).
    #[cfg(test)]
    fn new(text: &str) -> Self {
        Self::from_owned(text.to_string())
    }

    fn from_owned(text: String) -> Self {
        let trimmed = py_trim(&text);
        let text = if trimmed.len() == text.len() { text } else { trimmed.to_string() };
        let len = text.len();
        Self { pos: 0, len, text }
    }

    /// 跳过 Python `str.isspace()` 认定的空白（含 \x1c-\x1f）
    fn skip_space(&mut self) {
        while self.pos < self.len {
            match self.text[self.pos..].chars().next() {
                Some(c) if py_isspace(c) => self.pos += c.len_utf8(),
                _ => break,
            }
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.skip_space();
        self.text[self.pos..].chars().next()
    }

    fn next_token(&mut self) -> Option<String> {
        self.skip_space();
        if self.pos >= self.len {
            return None;
        }

        let ch = self.text[self.pos..].chars().next().unwrap();

        // 控制序列 / 命令 (e.g. \frac, \alpha)
        if ch == '\\' {
            // Python slices `str` by code point; a 1-byte advance would strand `pos`
            // mid-char on non-ASCII input (`\中`, `\，`, `\é`) and panic the next slice.
            let start = self.pos;
            self.pos += 1;
            if self.pos < self.len {
                let next = self.text[self.pos..].chars().next().unwrap();
                if !next.is_alphabetic() {
                    self.pos += next.len_utf8();
                    return Some(self.text[start..self.pos].to_string());
                }
                while self.pos < self.len {
                    let c = self.text[self.pos..].chars().next().unwrap();
                    if !c.is_alphabetic() {
                        break;
                    }
                    self.pos += c.len_utf8();
                }
            }
            return Some(self.text[start..self.pos].to_string());
        }

        // 单字符操作符/定界符
        self.pos += ch.len_utf8();
        Some(ch.to_string())
    }

    fn get_group(&mut self, open_ch: char, close_ch: char) -> String {
        self.skip_space();
        if self.pos >= self.len || self.text[self.pos..].chars().next().unwrap() != open_ch {
            // 如果不是以 open_ch 开头，提取单个 token
            return self.next_token().unwrap_or_default();
        }

        self.pos += open_ch.len_utf8(); // 跳过 open_ch
        let mut depth = 1;
        let start = self.pos;
        while self.pos < self.len {
            let c = self.text[self.pos..].chars().next().unwrap();
            if c == open_ch {
                depth += 1;
            } else if c == close_ch {
                depth -= 1;
                if depth == 0 {
                    let res = self.text[start..self.pos].to_string();
                    self.pos += c.len_utf8();
                    return res;
                }
            }
            self.pos += c.len_utf8();
        }
        self.text[start..].to_string()
    }
}

/// Python `str.isspace()`：Unicode White_Space 再加 U+001C..U+001F
/// （Rust 的 `char::is_whitespace` 不含这 4 个 C0 分隔符）。
fn py_isspace(ch: char) -> bool {
    ch.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&ch)
}

/// Python `str.strip()`（无参）语义
fn py_trim(text: &str) -> &str {
    text.trim_matches(py_isspace)
}

/// Python `text[drop:-drop]` 切片语义：越界返回空串而不是 panic
fn py_strip_slice(text: &str, drop: usize) -> &str {
    let end = text.len().saturating_sub(drop);
    if end <= drop {
        ""
    } else {
        &text[drop..end]
    }
}

/// Python `tok.next_token() or default`：None 与空串都回落到 default
fn next_token_or(tok: &mut LatexTokenizer, default: &str) -> String {
    let token = tok.next_token().unwrap_or_default();
    if token.is_empty() {
        default.to_string()
    } else {
        token
    }
}

/// 生成 OMML 文本 Run 节点
fn r(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let escaped = escape_xml(text);
    format!("<m:r><m:t>{}</m:t></m:r>", escaped)
}

/// 包装为 OMML 元素容器 <m:e>
fn e(inner: &str) -> String {
    format!("<m:e>{}</m:e>", inner)
}

/// XML 转义
///
/// 与 Python `xml.sax.saxutils.escape()` 完全一致：只处理 `&`、`<`、`>`，
/// 不转义引号（saxutils.escape 没有 entities 参数）。
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ------------------------------------------------------------- iterative parse
//
// `src/readmd_modules/latex2omml.py::_parse_latex_to_omml_inner` recurses once per
// nested group.  Measured on this box (CPython 3.11.15, default
// `recursionlimit=1000`, bisected through the public `latex_to_omml`): a `\frac`
// chain is accepted down to **993** nested groups and raises `RecursionError` at
// **994** — and the ceiling is *not* a fixed number: 941 with 50 frames already on
// the stack, 981 with 10.  `RecursionError` is a subclass of `Exception`, and the
// only caller, `mdexport/docx_render.py:475-499` (and its inline twin at
// `:151-158`), catches it and degrades to an italic run of the raw LaTeX source.
//
// That failure path cannot be ported.  `latex_to_omml` returns a plain `String`,
// and `mdexport.rs:2389`/`mdexport.rs:2432` splice it straight into the DOCX with
// no error channel: there is no error value the Rust caller already handles, so
// inventing one would mean changing a signature other lanes' code consumes, and
// returning anything shorter than the real render would silently delete the user's
// formula.  The recursion is therefore removed instead of bounded: one `Frame` per
// nesting level on the heap, carrying exactly the locals a Python frame carried.
// Every depth Python renders, this renders identically — and beyond Python's
// ceiling this still renders the formula rather than losing it.

/// A group one `\begin{…}` environment still owes, in the order Python descends
/// into it.
enum Item {
    /// One matrix cell: wrapped in `<m:e>` and collected into an `<m:mr>`.
    Cell(String),
    /// End of a matrix row.
    EndRow,
    /// One whole `aligned`/`cases` row, `&` already replaced by spaces.
    Row(String),
}

/// The construct a frame is inside of while one of its child groups is being
/// parsed — Python's frame locals between the descent and the point of use.
enum Blocked {
    FracNum { den: String },
    FracDen { num: String },
    BinomN { k: String },
    BinomK { n: String },
    /// `\sqrt[n]{x}` parses the radicand first and the index second, but emits
    /// `<m:deg>` first, so the raw index text must survive the first descent.
    RadBody { deg: String },
    RadDeg { body: String },
    NarySub,
    NarySup,
    Delim { beg: String, end: String },
    Acc { chr: &'static str },
    Boxed,
    Bar,
    LimLow { name: String },
    /// `x^{…}`.  Python pops the base off `out` *after* the descent, but nothing
    /// else touches `out` while the child is in flight, so popping it before the
    /// descent yields the same bytes.
    Sup { base: String },
    Sub { base: String },
    SubSup { base: String, sub: String },
    Cell,
    Row,
}

/// Live while a `\sum_a^b` limit list is being consumed.
struct Nary {
    chr: &'static str,
    sum: bool,
    sub: String,
    sup: String,
}

/// Live while a `\begin{…}` environment is consuming its groups.  Python builds
/// `m_rows_xml` for *every* environment — `aligned`/`cases` also do it and then
/// throw it away — so `mr_rows` and `eq_rows` are both kept here.
struct Table {
    env: String,
    queue: VecDeque<Item>,
    mr_rows: Vec<String>,
    cur: Vec<String>,
    eq_rows: Vec<String>,
}

/// One nesting level of the parse: the tokenizer for one group, the nodes built
/// from it so far, and which construct is waiting for a child.
struct Frame {
    tok: LatexTokenizer,
    out: Vec<String>,
    blocked: Option<Blocked>,
    nary: Option<Nary>,
    table: Option<Table>,
}

impl Frame {
    fn new(src: String) -> Self {
        Self {
            tok: LatexTokenizer::from_owned(src),
            out: Vec::new(),
            blocked: None,
            nary: None,
            table: None,
        }
    }

    /// Run the token loop until the next group has to be parsed — `Some(src)`, with
    /// `blocked` already saying what to do with the answer — or until the group is
    /// exhausted (`None`).
    fn run(&mut self) -> Option<String> {
        loop {
            let t = match self.tok.next_token() {
                Some(token) => token,
                None => return None,
            };

            // 1. 分数 \frac{num}{den}, \dfrac{num}{den}, \binom{n}{k}
            if t == r"\frac" || t == r"\dfrac" {
                let num = self.tok.get_group('{', '}');
                let den = self.tok.get_group('{', '}');
                self.blocked = Some(Blocked::FracNum { den });
                return Some(num);
            }

            if t == r"\binom" {
                let n = self.tok.get_group('{', '}');
                let k = self.tok.get_group('{', '}');
                self.blocked = Some(Blocked::BinomN { k });
                return Some(n);
            }

            // 2. 根式 \sqrt[n]{x}
            if t == r"\sqrt" {
                let mut deg_val = String::new();
                if self.tok.peek() == Some('[') {
                    deg_val = self.tok.get_group('[', ']');
                }
                let rad_body = self.tok.get_group('{', '}');
                self.blocked = Some(Blocked::RadBody { deg: deg_val });
                return Some(rad_body);
            }

            // 3. 大型运算符 \sum, \int 等
            if let Some(&op_char) = NARY_OPS.get(t.as_str()) {
                self.nary = Some(Nary {
                    chr: op_char,
                    sum: t == r"\sum",
                    sub: String::new(),
                    sup: String::new(),
                });
                match self.nary_more() {
                    Some(src) => return Some(src),
                    None => continue,
                }
            }

            // 4. 定界符 \left( ... \right), \left[ ... \right], \left\{ ... \right\}
            if t == r"\left" {
                let mut beg_delim = next_token_or(&mut self.tok, "(");
                if beg_delim == "\\" {
                    beg_delim = self.tok.next_token().unwrap_or_default();
                }
                if beg_delim == "{" {
                    beg_delim = "{".to_string();
                } else if beg_delim == "." {
                    beg_delim = "".to_string();
                }

                // 寻找对应的 \right
                let mut inner_tokens: Vec<String> = Vec::new();
                let mut depth = 1;
                let mut end_delim = ")".to_string();
                loop {
                    match self.tok.next_token() {
                        None => break,
                        Some(nxt) => {
                            if nxt == r"\left" {
                                depth += 1;
                                inner_tokens.push(nxt);
                            } else if nxt == r"\right" {
                                depth -= 1;
                                if depth == 0 {
                                    end_delim = next_token_or(&mut self.tok, ")");
                                    if end_delim == "\\" {
                                        end_delim = self.tok.next_token().unwrap_or_default();
                                    }
                                    if end_delim == "}" {
                                        end_delim = "}".to_string();
                                    } else if end_delim == "." {
                                        end_delim = "".to_string();
                                    }
                                    break;
                                } else {
                                    inner_tokens.push(nxt);
                                }
                            } else {
                                inner_tokens.push(nxt);
                            }
                        }
                    }
                }

                self.blocked = Some(Blocked::Delim { beg: beg_delim, end: end_delim });
                return Some(inner_tokens.join(" "));
            }

            // 5. 矩阵环境 \begin{matrix}, \begin{pmatrix}, \begin{bmatrix},
            //    \begin{aligned}
            if t == r"\begin" {
                self.begin_env();
                match self.table_more() {
                    Some(src) => return Some(src),
                    None => continue,
                }
            }

            // 6. 重音符号 \vec, \hat, \dot 等
            if let Some(&acc_char) = ACCENTS.get(t.as_str()) {
                let body = self.tok.get_group('{', '}');
                self.blocked = Some(Blocked::Acc { chr: acc_char });
                return Some(body);
            }

            // 7. \boxed{...} 与 \overline{...}
            if t == r"\boxed" {
                let body = self.tok.get_group('{', '}');
                self.blocked = Some(Blocked::Boxed);
                return Some(body);
            }

            if t == r"\overline" || t == r"\bar" {
                let body = self.tok.get_group('{', '}');
                self.blocked = Some(Blocked::Bar);
                return Some(body);
            }

            // 字体与文本包装 \text, \mathrm, \mathbf —— 无递归：Python 直接把
            // group 原文转义进 <m:t>。
            if matches!(
                t.as_str(),
                r"\text"
                    | r"\mathrm"
                    | r"\mathbf"
                    | r"\mathbb"
                    | r"\mathcal"
                    | r"\boldsymbol"
            ) {
                let body = self.tok.get_group('{', '}');
                if t == r"\text" {
                    self.out.push(format!(
                        "<m:r><m:rPr><m:nor/></m:rPr><m:t>{}</m:t></m:r>",
                        escape_xml(&body)
                    ));
                } else if t == r"\mathbf" || t == r"\boldsymbol" {
                    self.out.push(format!(
                        "<m:r><m:rPr><m:b/></m:rPr><m:t>{}</m:t></m:r>",
                        escape_xml(&body)
                    ));
                } else if t == r"\mathrm" {
                    self.out.push(format!(
                        "<m:r><m:rPr><m:i m:val=\"off\"/></m:rPr><m:t>{}</m:t></m:r>",
                        escape_xml(&body)
                    ));
                } else {
                    self.out.push(r(&body));
                }
                continue;
            }

            // 8. 常见函数名称 \sin, \cos, \ln, \lim
            if FUNCTIONS.contains(t.as_str()) {
                let fname = &t[1..];
                if t == r"\lim"
                    || t == r"\max"
                    || t == r"\min"
                    || t == r"\inf"
                    || t == r"\sup"
                {
                    // 检查是否有下标 \lim_{x \to 0}
                    if self.tok.peek() == Some('_') {
                        self.tok.next_token();
                        let sub_val = self.tok.get_group('{', '}');
                        // Python builds `_r(fname)` after the descent; `_r` is
                        // pure, so doing it first is the same bytes.
                        self.blocked = Some(Blocked::LimLow { name: r(fname) });
                        return Some(sub_val);
                    }
                }
                self.out.push(r(&format!("{} ", fname)));
                continue;
            }

            // 9. 符号映射表转换
            if let Some(&symbol) = MATH_SYMBOLS.get(t.as_str()) {
                self.out.push(r(symbol));
                continue;
            }

            // 10. 上标 ^ 与 下标 _
            if t == "^" {
                let sup_val = self.tok.get_group('{', '}');
                // 取出前一个已生成的节点作为基底
                let prev_base = self.out.pop().unwrap_or_else(|| r(""));
                self.blocked = Some(Blocked::Sup { base: prev_base });
                return Some(sup_val);
            }

            if t == "_" {
                let sub_val = self.tok.get_group('{', '}');
                let prev_base = self.out.pop().unwrap_or_else(|| r(""));
                self.blocked = Some(Blocked::Sub { base: prev_base });
                return Some(sub_val);
            }

            // 11. 普通文本与数字
            if t.starts_with('\\') {
                // 未识别命令：去除斜杠兜底
                self.out.push(r(&t[1..]));
            } else {
                self.out.push(r(&t));
            }
        }
    }

    /// Hand a finished child group's OMML to the construct that asked for it.
    /// `Some(src)` means the same construct immediately descends into another
    /// group; `None` means the node is complete and the token loop resumes.
    fn deliver(&mut self, val: String) -> Option<String> {
        match self.blocked.take().expect("a frame only descends with a construct") {
            Blocked::FracNum { den } => {
                self.blocked = Some(Blocked::FracDen { num: val });
                Some(den)
            }
            Blocked::FracDen { num } => {
                self.out.push(format!(
                    "<m:f><m:num>{}</m:num><m:den>{}</m:den></m:f>",
                    e(&num),
                    e(&val)
                ));
                None
            }
            Blocked::BinomN { k } => {
                self.blocked = Some(Blocked::BinomK { n: val });
                Some(k)
            }
            Blocked::BinomK { n } => {
                // 微软 OMML 中无横线分数 + 外层圆括号定界符
                let f_xml = format!(
                    "<m:f><m:fPr><m:type m:val=\"noBar\"/></m:fPr><m:num>{}</m:num><m:den>{}</m:den></m:f>",
                    e(&n),
                    e(&val)
                );
                self.out.push(format!(
                    "<m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e>{}</m:e></m:d>",
                    f_xml
                ));
                None
            }
            Blocked::RadBody { deg } => {
                if !deg.is_empty() {
                    self.blocked = Some(Blocked::RadDeg { body: val });
                    return Some(deg);
                }
                self.out.push(format!(
                    "<m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/><m:e>{}</m:e></m:rad>",
                    val
                ));
                None
            }
            Blocked::RadDeg { body } => {
                self.out.push(format!(
                    "<m:rad><m:deg>{}</m:deg><m:e>{}</m:e></m:rad>",
                    e(&val),
                    body
                ));
                None
            }
            Blocked::NarySub => {
                if let Some(n) = self.nary.as_mut() {
                    n.sub = val;
                }
                self.nary_more()
            }
            Blocked::NarySup => {
                if let Some(n) = self.nary.as_mut() {
                    n.sup = val;
                }
                self.nary_more()
            }
            Blocked::Delim { beg, end } => {
                let mut dpr = "<m:dPr>".to_string();
                if !beg.is_empty() {
                    dpr.push_str(&format!("<m:begChr m:val=\"{}\"/>", escape_xml(&beg)));
                } else {
                    dpr.push_str("<m:begChr m:val=\"\"/>");
                }
                if !end.is_empty() {
                    dpr.push_str(&format!("<m:endChr m:val=\"{}\"/>", escape_xml(&end)));
                } else {
                    dpr.push_str("<m:endChr m:val=\"\"/>");
                }
                dpr.push_str("</m:dPr>");
                self.out.push(format!("<m:d>{}<m:e>{}</m:e></m:d>", dpr, val));
                None
            }
            Blocked::Acc { chr } => {
                self.out.push(format!(
                    "<m:acc><m:accPr><m:chr m:val=\"{}\"/></m:accPr><m:e>{}</m:e></m:acc>",
                    escape_xml(chr),
                    val
                ));
                None
            }
            Blocked::Boxed => {
                self.out.push(format!("<m:borderBox><m:e>{}</m:e></m:borderBox>", val));
                None
            }
            Blocked::Bar => {
                self.out.push(format!(
                    "<m:bar><m:barPr><m:pos m:val=\"top\"/></m:barPr><m:e>{}</m:e></m:bar>",
                    val
                ));
                None
            }
            Blocked::LimLow { name } => {
                self.out.push(format!(
                    "<m:limLow><m:e>{}</m:e><m:lim>{}</m:lim></m:limLow>",
                    name,
                    e(&val)
                ));
                None
            }
            Blocked::Sup { base } => {
                self.out.push(format!(
                    "<m:sSup><m:e>{}</m:e><m:sup>{}</m:sup></m:sSup>",
                    base,
                    e(&val)
                ));
                None
            }
            Blocked::Sub { base } => {
                // 检查紧随其后是否还有上标 x_i^2
                if self.tok.peek() == Some('^') {
                    self.tok.next_token();
                    let sup_val = self.tok.get_group('{', '}');
                    self.blocked = Some(Blocked::SubSup { base, sub: val });
                    return Some(sup_val);
                }
                self.out.push(format!(
                    "<m:sSub><m:e>{}</m:e><m:sub>{}</m:sub></m:sSub>",
                    base,
                    e(&val)
                ));
                None
            }
            Blocked::SubSup { base, sub } => {
                self.out.push(format!(
                    "<m:sSubSup><m:e>{}</m:e><m:sub>{}</m:sub><m:sup>{}</m:sup></m:sSubSup>",
                    base,
                    e(&sub),
                    e(&val)
                ));
                None
            }
            Blocked::Cell => {
                if let Some(tb) = self.table.as_mut() {
                    tb.cur.push(format!("<m:e>{}</m:e>", val));
                }
                self.table_more()
            }
            Blocked::Row => {
                if let Some(tb) = self.table.as_mut() {
                    tb.eq_rows.push(format!("<m:e>{}</m:e>", val));
                }
                self.table_more()
            }
        }
    }

    /// The `while tok.peek() in ('_', '^')` loop of one `\sum` / `\int` node.
    fn nary_more(&mut self) -> Option<String> {
        loop {
            match self.tok.peek() {
                Some('_') => {
                    self.tok.next_token();
                    let src = self.tok.get_group('{', '}');
                    self.blocked = Some(Blocked::NarySub);
                    return Some(src);
                }
                Some('^') => {
                    self.tok.next_token();
                    let src = self.tok.get_group('{', '}');
                    self.blocked = Some(Blocked::NarySup);
                    return Some(src);
                }
                _ => {
                    self.finish_nary();
                    return None;
                }
            }
        }
    }

    fn finish_nary(&mut self) {
        let n = self.nary.take().expect("nary_more only runs inside \\sum-like ops");
        let chr_attr = format!("<m:chr m:val=\"{}\"/>", escape_xml(n.chr));
        let lim_loc = if n.sum {
            "<m:limLoc m:val=\"undOvr\"/>"
        } else {
            "<m:limLoc m:val=\"subSup\"/>"
        };
        let nary_pr = format!("<m:naryPr>{}{}</m:naryPr>", chr_attr, lim_loc);
        let sub_part = if !n.sub.is_empty() {
            format!("<m:sub>{}</m:sub>", e(&n.sub))
        } else {
            "<m:sub/>".to_string()
        };
        let sup_part = if !n.sup.is_empty() {
            format!("<m:sup>{}</m:sup>", e(&n.sup))
        } else {
            "<m:sup/>".to_string()
        };
        self.out.push(format!(
            "<m:nary>{}{}{}<m:e/></m:nary>",
            nary_pr, sub_part, sup_part
        ));
    }

    /// `\begin{env}`: cut the body at `\end{env}` and lay out every group Python
    /// would descend into, in Python's order (all cells first, then — for
    /// `aligned`/`cases`, which build *and discard* the cell rows — all rows).
    fn begin_env(&mut self) {
        let env = py_trim(&self.tok.get_group('{', '}')).to_string();
        // 收集环境内容直到 \end{env}
        let end_cmd = format!(r"\end{{{}}}", env);
        let env_start = self.tok.pos;
        let end_idx = self.tok.text[env_start..].find(&end_cmd).map(|offset| env_start + offset);
        let env_body = match end_idx {
            Some(idx) => {
                let body = py_trim(&self.tok.text[env_start..idx]).to_string();
                self.tok.pos = idx + end_cmd.len();
                body
            }
            None => {
                let body = py_trim(&self.tok.text[env_start..]).to_string();
                self.tok.pos = self.tok.len;
                body
            }
        };

        // 解析矩阵行与列
        let rows: Vec<String> = env_body.split(r"\\").map(|r| py_trim(r).to_string()).collect();
        let mut queue: VecDeque<Item> = VecDeque::new();
        for row in rows.iter() {
            if row.is_empty() {
                continue;
            }
            for cell in row.split('&') {
                queue.push_back(Item::Cell(py_trim(cell).to_string()));
            }
            queue.push_back(Item::EndRow);
        }
        if matches!(env.as_str(), "aligned" | "cases") {
            for row in rows.iter() {
                if row.is_empty() {
                    continue;
                }
                queue.push_back(Item::Row(row.replace('&', " ")));
            }
        }
        self.table = Some(Table {
            env,
            queue,
            mr_rows: Vec::new(),
            cur: Vec::new(),
            eq_rows: Vec::new(),
        });
    }

    /// Pop the next group of the open environment, closing rows as their cells
    /// come in, and finish the `<m:m>` / `<m:eqArr>` node once the queue is dry.
    fn table_more(&mut self) -> Option<String> {
        if self.table.is_none() {
            return None;
        }
        loop {
            let next = match self.table.as_mut().map(|tb| tb.queue.pop_front()) {
                Some(Some(item)) => item,
                _ => break,
            };
            match next {
                Item::Cell(src) => {
                    self.blocked = Some(Blocked::Cell);
                    return Some(src);
                }
                Item::Row(src) => {
                    self.blocked = Some(Blocked::Row);
                    return Some(src);
                }
                Item::EndRow => {
                    if let Some(tb) = self.table.as_mut() {
                        let cells = tb.cur.concat();
                        tb.cur.clear();
                        tb.mr_rows.push(format!("<m:mr>{}</m:mr>", cells));
                    }
                }
            }
        }
        self.finish_table();
        None
    }

    fn finish_table(&mut self) {
        let tb = self.table.take().expect("finish_table only runs with an open env");
        let m_inner = tb.mr_rows.concat();
        let eq_inner = tb.eq_rows.concat();
        match tb.env.as_str() {
            "matrix" => self.out.push(format!("<m:m>{}</m:m>", m_inner)),
            "pmatrix" => self.out.push(format!(
                "<m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:m>{}</m:m></m:e></m:d>",
                m_inner
            )),
            "bmatrix" => self.out.push(format!(
                r#"<m:d><m:dPr><m:begChr m:val="["/><m:endChr m:val="]"/></m:dPr><m:e><m:m>{}</m:m></m:e></m:d>"#,
                m_inner
            )),
            "vmatrix" => self.out.push(format!(
                "<m:d><m:dPr><m:begChr m:val=\"|\"/><m:endChr m:val=\"|\"/></m:dPr><m:e><m:m>{}</m:m></m:e></m:d>",
                m_inner
            )),
            "aligned" | "cases" => {
                // 方程组 / 多行对齐
                let eq_arr = format!("<m:eqArr>{}</m:eqArr>", eq_inner);
                if tb.env == "cases" {
                    self.out.push(format!(
                        r#"<m:d><m:dPr><m:begChr m:val="{{"/><m:endChr m:val=""/></m:dPr><m:e>{}</m:e></m:d>"#,
                        eq_arr
                    ));
                } else {
                    self.out.push(eq_arr);
                }
            }
            _ => self.out.push(format!("<m:m>{}</m:m>", m_inner)),
        }
    }
}

/// 将一段 LaTeX 解析为 OMML XML 内部节点片段——**迭代实现，栈深与嵌套层数无关**。
///
/// 见文件内注释：递归版本对每一层嵌套都要一个调用帧，`.md` 里几千层
/// `\frac{…}{…}` 会直接把 1 MiB / 2 MiB 的线程栈打穿。
fn parse_latex_to_omml_inner(latex_str: &str) -> String {
    if latex_str.is_empty() {
        return String::new();
    }

    // The frame stack *is* the call stack, moved to the heap.
    let mut stack: Vec<Frame> = Vec::new();
    stack.push(Frame::new(latex_str.to_string()));
    // OMML produced by the frame that just finished, waiting for its parent.
    let mut ready: Option<String> = None;

    loop {
        // `resumed` distinguishes the two ways an iteration can end without a
        // descent: the parent just absorbed a child's OMML and still has tokens
        // left (this is where the recursive call used to return into), or the
        // frame really has consumed its whole group.
        let (descend, resumed) = {
            let top = stack.last_mut().expect("stack is never empty");
            match ready.take() {
                Some(val) => (top.deliver(val), true),
                None => (top.run(), false),
            }
        };
        if let Some(src) = descend {
            stack.push(Frame::new(src));
            continue;
        }
        if resumed {
            continue;
        }
        let finished = stack.pop().expect("stack is never empty");
        if stack.is_empty() {
            return finished.out.join("");
        }
        ready = Some(finished.out.join(""));
    }
}

/// 递归将 LaTeX 字符串解析为 OMML XML 内部节点片段。
///
/// The verbatim recursive implementation that [`parse_latex_to_omml_inner`]
/// replaced, kept under `cfg(test)` as a differential oracle — the suite asserts
/// the frame-stack version produces byte-identical OMML, so the rewrite provably
/// changed no output.  It is *not* reachable from `latex_to_omml` any more.
#[cfg(test)]
fn parse_latex_to_omml_inner_recursive(latex_str: &str) -> String {
    if latex_str.is_empty() {
        return String::new();
    }

    let mut tok = LatexTokenizer::new(latex_str);
    let mut out: Vec<String> = Vec::new();

    loop {
        let t = match tok.next_token() {
            Some(token) => token,
            None => break,
        };

        // 1. 分数 \frac{num}{den}, \dfrac{num}{den}, \binom{n}{k}
        if t == r"\frac" || t == r"\dfrac" {
            let num = tok.get_group('{', '}');
            let den = tok.get_group('{', '}');
            let num_omml = parse_latex_to_omml_inner_recursive(&num);
            let den_omml = parse_latex_to_omml_inner_recursive(&den);
            out.push(format!(
                "<m:f><m:num>{}</m:num><m:den>{}</m:den></m:f>",
                e(&num_omml),
                e(&den_omml)
            ));
            continue;
        }

        if t == r"\binom" {
            let n = tok.get_group('{', '}');
            let k = tok.get_group('{', '}');
            let n_omml = parse_latex_to_omml_inner_recursive(&n);
            let k_omml = parse_latex_to_omml_inner_recursive(&k);
            // 微软 OMML 中无横线分数 + 外层圆括号定界符
            let f_xml = format!(
                "<m:f><m:fPr><m:type m:val=\"noBar\"/></m:fPr><m:num>{}</m:num><m:den>{}</m:den></m:f>",
                e(&n_omml),
                e(&k_omml)
            );
            out.push(format!(
                "<m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e>{}</m:e></m:d>",
                f_xml
            ));
            continue;
        }

        // 2. 根式 \sqrt[n]{x}
        if t == r"\sqrt" {
            let mut deg_val = String::new();
            if tok.peek() == Some('[') {
                deg_val = tok.get_group('[', ']');
            }
            let rad_body = tok.get_group('{', '}');
            let body_omml = parse_latex_to_omml_inner_recursive(&rad_body);
            if !deg_val.is_empty() {
                let deg_omml = parse_latex_to_omml_inner_recursive(&deg_val);
                out.push(format!(
                    "<m:rad><m:deg>{}</m:deg><m:e>{}</m:e></m:rad>",
                    e(&deg_omml),
                    body_omml
                ));
            } else {
                out.push(format!(
                    "<m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/><m:e>{}</m:e></m:rad>",
                    body_omml
                ));
            }
            continue;
        }

        // 3. 大型运算符 \sum, \int 等
        if let Some(&op_char) = NARY_OPS.get(t.as_str()) {
            // 检查后续是否有上下标
            let mut sub_xml = String::new();
            let mut sup_xml = String::new();
            loop {
                match tok.peek() {
                    Some('_') => {
                        tok.next_token();
                        let sub_val = tok.get_group('{', '}');
                        sub_xml = parse_latex_to_omml_inner_recursive(&sub_val);
                    }
                    Some('^') => {
                        tok.next_token();
                        let sup_val = tok.get_group('{', '}');
                        sup_xml = parse_latex_to_omml_inner_recursive(&sup_val);
                    }
                    _ => break,
                }
            }

            let chr_attr = format!("<m:chr m:val=\"{}\"/>", escape_xml(&op_char));
            let lim_loc = if t == r"\sum" {
                "<m:limLoc m:val=\"undOvr\"/>"
            } else {
                "<m:limLoc m:val=\"subSup\"/>"
            };
            let nary_pr = format!("<m:naryPr>{}{}</m:naryPr>", chr_attr, lim_loc);
            let sub_part = if !sub_xml.is_empty() {
                format!("<m:sub>{}</m:sub>", e(&sub_xml))
            } else {
                "<m:sub/>".to_string()
            };
            let sup_part = if !sup_xml.is_empty() {
                format!("<m:sup>{}</m:sup>", e(&sup_xml))
            } else {
                "<m:sup/>".to_string()
            };
            out.push(format!(
                "<m:nary>{}{}{}<m:e/></m:nary>",
                nary_pr, sub_part, sup_part
            ));
            continue;
        }

        // 4. 定界符 \left( ... \right), \left[ ... \right], \left\{ ... \right\}
        if t == r"\left" {
            let mut beg_delim = next_token_or(&mut tok, "(");
            if beg_delim == "\\" {
                beg_delim = tok.next_token().unwrap_or_default();
            }
            if beg_delim == "{" {
                beg_delim = "{".to_string();
            } else if beg_delim == "." {
                beg_delim = "".to_string();
            }

            // 寻找对应的 \right
            let mut inner_tokens: Vec<String> = Vec::new();
            let mut depth = 1;
            let mut end_delim = ")".to_string();
            loop {
                match tok.next_token() {
                    None => break,
                    Some(nxt) => {
                        if nxt == r"\left" {
                            depth += 1;
                            inner_tokens.push(nxt);
                        } else if nxt == r"\right" {
                            depth -= 1;
                            if depth == 0 {
                                end_delim = next_token_or(&mut tok, ")");
                                if end_delim == "\\" {
                                    end_delim = tok.next_token().unwrap_or_default();
                                }
                                if end_delim == "}" {
                                    end_delim = "}".to_string();
                                } else if end_delim == "." {
                                    end_delim = "".to_string();
                                }
                                break;
                            } else {
                                inner_tokens.push(nxt);
                            }
                        } else {
                            inner_tokens.push(nxt);
                        }
                    }
                }
            }

            let inner_content = inner_tokens.join(" ");
            let inner_omml = parse_latex_to_omml_inner_recursive(&inner_content);
            let mut dpr = "<m:dPr>".to_string();
            if !beg_delim.is_empty() {
                dpr.push_str(&format!("<m:begChr m:val=\"{}\"/>", escape_xml(&beg_delim)));
            } else {
                dpr.push_str("<m:begChr m:val=\"\"/>");
            }
            if !end_delim.is_empty() {
                dpr.push_str(&format!("<m:endChr m:val=\"{}\"/>", escape_xml(&end_delim)));
            } else {
                dpr.push_str("<m:endChr m:val=\"\"/>");
            }
            dpr.push_str("</m:dPr>");
            out.push(format!("<m:d>{}<m:e>{}</m:e></m:d>", dpr, inner_omml));
            continue;
        }

        // 5. 矩阵环境 \begin{matrix}, \begin{pmatrix}, \begin{bmatrix}, \begin{aligned}
        if t == r"\begin" {
            let env = py_trim(&tok.get_group('{', '}')).to_string();
            // 收集环境内容直到 \end{env}
            let end_cmd = format!(r"\end{{{}}}", env);
            let env_start = tok.pos;
            let end_idx = tok.text[env_start..]
                .find(&end_cmd)
                .map(|offset| env_start + offset);
            let env_body = if let Some(idx) = end_idx {
                let body = py_trim(&tok.text[env_start..idx]);
                tok.pos = idx + end_cmd.len();
                body
            } else {
                let body = py_trim(&tok.text[env_start..]);
                tok.pos = tok.len;
                body
            };

            // 解析矩阵行与列
            let rows: Vec<&str> = env_body.split(r"\\").collect();
            let mut m_rows_xml: Vec<String> = Vec::new();
            for row in rows.iter() {
                let row = py_trim(row);
                if row.is_empty() {
                    continue;
                }
                let cells: Vec<&str> = row.split('&').collect();
                let cells_xml: String = cells
                    .iter()
                    .map(|c| format!("<m:e>{}</m:e>", parse_latex_to_omml_inner_recursive(py_trim(c))))
                    .collect();
                m_rows_xml.push(format!("<m:mr>{}</m:mr>", cells_xml));
            }

            let m_inner = m_rows_xml.join("");
            match env.as_str() {
                "matrix" => out.push(format!("<m:m>{}</m:m>", m_inner)),
                "pmatrix" => out.push(format!(
                    "<m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:m>{}</m:m></m:e></m:d>",
                    m_inner
                )),
                "bmatrix" => out.push(format!(
                    r#"<m:d><m:dPr><m:begChr m:val="["/><m:endChr m:val="]"/></m:dPr><m:e><m:m>{}</m:m></m:e></m:d>"#,
                    m_inner
                )),
                "vmatrix" => out.push(format!(
                    "<m:d><m:dPr><m:begChr m:val=\"|\"/><m:endChr m:val=\"|\"/></m:dPr><m:e><m:m>{}</m:m></m:e></m:d>",
                    m_inner
                )),
                "aligned" | "cases" => {
                    // 方程组 / 多行对齐
                    let eq_arr: String = rows
                        .iter()
                        .filter_map(|r| {
                            let r = py_trim(r);
                            if r.is_empty() {
                                None
                            } else {
                                Some(format!(
                                    "<m:e>{}</m:e>",
                                    parse_latex_to_omml_inner_recursive(&r.replace('&', " "))
                                ))
                            }
                        })
                        .collect();
                    if env == "cases" {
                        out.push(format!(
                            r#"<m:d><m:dPr><m:begChr m:val="{{"/><m:endChr m:val=""/></m:dPr><m:e>{}</m:e></m:d>"#,
                            format!("<m:eqArr>{}</m:eqArr>", eq_arr)
                        ));
                    } else {
                        out.push(format!("<m:eqArr>{}</m:eqArr>", eq_arr));
                    }
                }
                _ => out.push(format!("<m:m>{}</m:m>", m_inner)),
            }
            continue;
        }

        // 6. 重音符号 \vec, \hat, \dot 等
        if let Some(&acc_char) = ACCENTS.get(t.as_str()) {
            let body = tok.get_group('{', '}');
            let body_omml = parse_latex_to_omml_inner_recursive(&body);
            out.push(format!(
                "<m:acc><m:accPr><m:chr m:val=\"{}\"/></m:accPr><m:e>{}</m:e></m:acc>",
                escape_xml(acc_char),
                body_omml
            ));
            continue;
        }

        // 7. \boxed{...} 与 \overline{...}
        if t == r"\boxed" {
            let body = tok.get_group('{', '}');
            let body_omml = parse_latex_to_omml_inner_recursive(&body);
            out.push(format!("<m:borderBox><m:e>{}</m:e></m:borderBox>", body_omml));
            continue;
        }

        if t == r"\overline" || t == r"\bar" {
            let body = tok.get_group('{', '}');
            let body_omml = parse_latex_to_omml_inner_recursive(&body);
            out.push(format!(
                "<m:bar><m:barPr><m:pos m:val=\"top\"/></m:barPr><m:e>{}</m:e></m:bar>",
                body_omml
            ));
            continue;
        }

        // 字体与文本包装 \text, \mathrm, \mathbf
        if matches!(
            t.as_str(),
            r"\text"
                | r"\mathrm"
                | r"\mathbf"
                | r"\mathbb"
                | r"\mathcal"
                | r"\boldsymbol"
        ) {
            let body = tok.get_group('{', '}');
            if t == r"\text" {
                out.push(format!(
                    "<m:r><m:rPr><m:nor/></m:rPr><m:t>{}</m:t></m:r>",
                    escape_xml(&body)
                ));
            } else if t == r"\mathbf" || t == r"\boldsymbol" {
                out.push(format!(
                    "<m:r><m:rPr><m:b/></m:rPr><m:t>{}</m:t></m:r>",
                    escape_xml(&body)
                ));
            } else if t == r"\mathrm" {
                out.push(format!(
                    "<m:r><m:rPr><m:i m:val=\"off\"/></m:rPr><m:t>{}</m:t></m:r>",
                    escape_xml(&body)
                ));
            } else {
                out.push(r(&body));
            }
            continue;
        }

        // 8. 常见函数名称 \sin, \cos, \ln, \lim
        if FUNCTIONS.contains(t.as_str()) {
            let fname = &t[1..];
            if t == r"\lim"
                || t == r"\max"
                || t == r"\min"
                || t == r"\inf"
                || t == r"\sup"
            {
                // 检查是否有下标 \lim_{x \to 0}
                if tok.peek() == Some('_') {
                    tok.next_token();
                    let sub_val = tok.get_group('{', '}');
                    let sub_omml = parse_latex_to_omml_inner_recursive(&sub_val);
                    let fname_omml = r(fname);
                    out.push(format!(
                        "<m:limLow><m:e>{}</m:e><m:lim>{}</m:lim></m:limLow>",
                        fname_omml,
                        e(&sub_omml)
                    ));
                    continue;
                }
            }
            out.push(r(&format!("{} ", fname)));
            continue;
        }

        // 9. 符号映射表转换
        if let Some(&symbol) = MATH_SYMBOLS.get(t.as_str()) {
            out.push(r(symbol));
            continue;
        }

        // 10. 上标 ^ 与 下标 _
        if t == "^" {
            let sup_val = tok.get_group('{', '}');
            let sup_omml = parse_latex_to_omml_inner_recursive(&sup_val);
            // 取出前一个已生成的节点作为基底
            let prev_base = out.pop().unwrap_or_else(|| r(""));
            out.push(format!(
                "<m:sSup><m:e>{}</m:e><m:sup>{}</m:sup></m:sSup>",
                prev_base,
                e(&sup_omml)
            ));
            continue;
        }

        if t == "_" {
            let sub_val = tok.get_group('{', '}');
            let sub_omml = parse_latex_to_omml_inner_recursive(&sub_val);
            let prev_base = out.pop().unwrap_or_else(|| r(""));
            // 检查紧随其后是否还有上标 x_i^2
            if tok.peek() == Some('^') {
                tok.next_token();
                let sup_val = tok.get_group('{', '}');
                let sup_omml = parse_latex_to_omml_inner_recursive(&sup_val);
                out.push(format!(
                    "<m:sSubSup><m:e>{}</m:e><m:sub>{}</m:sub><m:sup>{}</m:sup></m:sSubSup>",
                    prev_base,
                    e(&sub_omml),
                    e(&sup_omml)
                ));
            } else {
                out.push(format!(
                    "<m:sSub><m:e>{}</m:e><m:sub>{}</m:sub></m:sSub>",
                    prev_base,
                    e(&sub_omml)
                ));
            }
            continue;
        }

        // 11. 普通文本与数字
        if t.starts_with('\\') {
            // 未识别命令：去除斜杠兜底
            out.push(r(&t[1..]));
        } else {
            out.push(r(&t));
        }
    }

    out.join("")
}

/// 将 LaTeX 公式转为标准 OMML XML 字符串
///
/// # Arguments
///
/// * `latex_code` - LaTeX 公式文本（如 "E = mc^2" 或 "\\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}"）
/// * `is_block` - 是否为独立公式块（若为 True 则包裹为 <m:oMathPara>）
///
/// # Returns
///
/// 带有命名空间声明的完整 OMML XML 字符串
pub fn latex_to_omml(latex_code: &str, is_block: bool) -> String {
    let clean_latex = py_trim(latex_code);
    let mut is_block = is_block;

    // 移除外层 $ 或 $$（Python 切片越界时得到空串，不 panic）
    let clean_latex = if clean_latex.starts_with("$$") && clean_latex.ends_with("$$") {
        is_block = true;
        py_strip_slice(clean_latex, 2)
    } else if clean_latex.starts_with('$') && clean_latex.ends_with('$') {
        py_strip_slice(clean_latex, 1)
    } else {
        clean_latex
    };

    let inner_xml = parse_latex_to_omml_inner(py_trim(clean_latex));
    let inner_xml = if inner_xml.is_empty() {
        "<m:r><m:t></m:t></m:r>"
    } else {
        &inner_xml
    };

    let omath_xml = format!(
        "<m:oMath xmlns:m=\"{}\" xmlns:w=\"{}\">{}</m:oMath>",
        _M_NS, _W_NS, inner_xml
    );

    if is_block {
        format!(
            "<m:oMathPara xmlns:m=\"{}\" xmlns:w=\"{}\"><m:oMath>{}</m:oMath></m:oMathPara>",
            _M_NS, _W_NS, inner_xml
        )
    } else {
        omath_xml
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_formula() {
        let result = latex_to_omml("x + y", false);
        assert!(result.contains("<m:oMath"));
        assert!(result.contains("<m:t>x</m:t>"));
        assert!(result.contains("<m:t>y</m:t>"));
    }

    #[test]
    fn test_fraction() {
        let result = latex_to_omml(r"\frac{a}{b}", false);
        assert!(result.contains("<m:f>"));
        assert!(result.contains("<m:num>"));
        assert!(result.contains("<m:den>"));
    }

    #[test]
    fn test_superscript() {
        let result = latex_to_omml("x^2", false);
        assert!(result.contains("<m:sSup>"));
    }

    #[test]
    fn test_subscript() {
        let result = latex_to_omml("x_i", false);
        assert!(result.contains("<m:sSub>"));
    }

    #[test]
    fn test_greek_letters() {
        let result = latex_to_omml(r"\alpha + \beta", false);
        assert!(result.contains("α"));
        assert!(result.contains("β"));
    }

    #[test]
    fn test_multibyte_control_sequence_does_not_panic() {
        // `\` + a multi-byte char used to advance `pos` by one byte, stranding it
        // mid-char so the next `&text[pos..]` slice panicked.
        for src in ["\\中", "\\，", "\\é", "\\frac{中}{文}", "\\sqrt{ä}"] {
            let result = latex_to_omml(src, false);
            assert!(result.contains("<m:oMath"), "{src} => {result}");
        }
    }

    #[test]
    fn test_sqrt() {
        let result = latex_to_omml(r"\sqrt{x}", false);
        assert!(result.contains("<m:rad>"));
    }

    #[test]
    fn test_matrix() {
        let result = latex_to_omml(
            r"\begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix}",
            false,
        );
        assert!(result.contains("<m:d>"));
        assert!(result.contains("<m:mr>"));
    }

    #[test]
    fn test_summation() {
        let result = latex_to_omml(r"\sum_{i=1}^{n}", false);
        assert!(result.contains("<m:nary>"));
    }

    #[test]
    fn test_block_equation() {
        let result = latex_to_omml("$$E = mc^2$$", true);
        assert!(result.contains("<m:oMathPara"));
    }

    /// Python 派生的逐字符对照表（inline 模式）。
    ///
    /// 期望值全部由 `scratch/p6_omml_golden.py` 调用权威实现
    /// `src/readmd_modules/latex2omml.py::latex_to_omml` 生成后内联到此。
    /// 采集纯离线：不发网络请求、不起子进程、不启动 readmd 服务。
    #[test]
    fn omml_inline_matches_python_character_by_character() {
        let cases: &[(&str, &str)] = &[
        ("E = mc^2", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>E</m:t></m:r><m:r><m:t>=</m:t></m:r><m:r><m:t>m</m:t></m:r><m:sSup><m:e><m:r><m:t>c</m:t></m:r></m:e><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSup></m:oMath>"),
        ("\\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:f><m:num><m:e><m:r><m:t>-</m:t></m:r><m:r><m:t>b</m:t></m:r><m:r><m:t>±</m:t></m:r><m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/><m:e><m:sSup><m:e><m:r><m:t>b</m:t></m:r></m:e><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSup><m:r><m:t>-</m:t></m:r><m:r><m:t>4</m:t></m:r><m:r><m:t>a</m:t></m:r><m:r><m:t>c</m:t></m:r></m:e></m:rad></m:e></m:num><m:den><m:e><m:r><m:t>2</m:t></m:r><m:r><m:t>a</m:t></m:r></m:e></m:den></m:f></m:oMath>"),
        ("\\sum_{i=1}^{n} i^2", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:nary><m:naryPr><m:chr m:val=\"∑\"/><m:limLoc m:val=\"undOvr\"/></m:naryPr><m:sub><m:e><m:r><m:t>i</m:t></m:r><m:r><m:t>=</m:t></m:r><m:r><m:t>1</m:t></m:r></m:e></m:sub><m:sup><m:e><m:r><m:t>n</m:t></m:r></m:e></m:sup><m:e/></m:nary><m:sSup><m:e><m:r><m:t>i</m:t></m:r></m:e><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSup></m:oMath>"),
        ("\\int_0^1 x^2 dx", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:nary><m:naryPr><m:chr m:val=\"∫\"/><m:limLoc m:val=\"subSup\"/></m:naryPr><m:sub><m:e><m:r><m:t>0</m:t></m:r></m:e></m:sub><m:sup><m:e><m:r><m:t>1</m:t></m:r></m:e></m:sup><m:e/></m:nary><m:sSup><m:e><m:r><m:t>x</m:t></m:r></m:e><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSup><m:r><m:t>d</m:t></m:r><m:r><m:t>x</m:t></m:r></m:oMath>"),
        ("\\left( \\frac{a}{b} \\right)", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:f><m:num><m:e><m:r><m:t>a</m:t></m:r></m:e></m:num><m:den><m:e><m:r><m:t>b</m:t></m:r></m:e></m:den></m:f></m:e></m:d></m:oMath>"),
        ("\\left\\{ x \\right}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:d><m:dPr><m:begChr m:val=\"\\{\"/><m:endChr m:val=\"}\"/></m:dPr><m:e><m:r><m:t>x</m:t></m:r></m:e></m:d></m:oMath>"),
        ("\\left[ 0, 1 \\right]", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:d><m:dPr><m:begChr m:val=\"[\"/><m:endChr m:val=\"]\"/></m:dPr><m:e><m:r><m:t>0</m:t></m:r><m:r><m:t>,</m:t></m:r><m:r><m:t>1</m:t></m:r></m:e></m:d></m:oMath>"),
        ("\\left. \\frac{d}{dx} \\right|_{x=0}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:sSub><m:e><m:d><m:dPr><m:begChr m:val=\"\"/><m:endChr m:val=\"|\"/></m:dPr><m:e><m:f><m:num><m:e><m:r><m:t>d</m:t></m:r></m:e></m:num><m:den><m:e><m:r><m:t>d</m:t></m:r><m:r><m:t>x</m:t></m:r></m:e></m:den></m:f></m:e></m:d></m:e><m:sub><m:e><m:r><m:t>x</m:t></m:r><m:r><m:t>=</m:t></m:r><m:r><m:t>0</m:t></m:r></m:e></m:sub></m:sSub></m:oMath>"),
        ("\\left\\| x \\right\\|", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:d><m:dPr><m:begChr m:val=\"\\|\"/><m:endChr m:val=\"\\|\"/></m:dPr><m:e><m:r><m:t>x</m:t></m:r></m:e></m:d></m:oMath>"),
        ("\\left\\{ a \\right\\}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:d><m:dPr><m:begChr m:val=\"\\{\"/><m:endChr m:val=\"\\}\"/></m:dPr><m:e><m:r><m:t>a</m:t></m:r></m:e></m:d></m:oMath>"),
        ("\\left(a\\right)", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:r><m:t>a</m:t></m:r></m:e></m:d></m:oMath>"),
        ("\\left(x", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:r><m:t>x</m:t></m:r></m:e></m:d></m:oMath>"),
        ("\\right)", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>right</m:t></m:r><m:r><m:t>)</m:t></m:r></m:oMath>"),
        ("\\left(\\left(a\\right)\\right)", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:r><m:t>a</m:t></m:r></m:e></m:d></m:e></m:d></m:oMath>"),
        ("\\begin{pmatrix} 1 & 2 \\\\ 3 & 4 \\end{pmatrix}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:m><m:mr><m:e><m:r><m:t>1</m:t></m:r></m:e><m:e><m:r><m:t>2</m:t></m:r></m:e></m:mr><m:mr><m:e><m:r><m:t>3</m:t></m:r></m:e><m:e><m:r><m:t>4</m:t></m:r></m:e></m:mr></m:m></m:e></m:d></m:oMath>"),
        ("\\begin{bmatrix} a \\end{bmatrix}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:d><m:dPr><m:begChr m:val=\"[\"/><m:endChr m:val=\"]\"/></m:dPr><m:e><m:m><m:mr><m:e><m:r><m:t>a</m:t></m:r></m:e></m:mr></m:m></m:e></m:d></m:oMath>"),
        ("\\begin{vmatrix} 1 & 0 \\\\ 0 & 1 \\end{vmatrix}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:d><m:dPr><m:begChr m:val=\"|\"/><m:endChr m:val=\"|\"/></m:dPr><m:e><m:m><m:mr><m:e><m:r><m:t>1</m:t></m:r></m:e><m:e><m:r><m:t>0</m:t></m:r></m:e></m:mr><m:mr><m:e><m:r><m:t>0</m:t></m:r></m:e><m:e><m:r><m:t>1</m:t></m:r></m:e></m:mr></m:m></m:e></m:d></m:oMath>"),
        ("\\begin{matrix} 1 & 2 \\\\ 3 & 4 \\end{matrix}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:m><m:mr><m:e><m:r><m:t>1</m:t></m:r></m:e><m:e><m:r><m:t>2</m:t></m:r></m:e></m:mr><m:mr><m:e><m:r><m:t>3</m:t></m:r></m:e><m:e><m:r><m:t>4</m:t></m:r></m:e></m:mr></m:m></m:oMath>"),
        ("\\begin{cases} x & y \\\\ z & w \\end{cases}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:d><m:dPr><m:begChr m:val=\"{\"/><m:endChr m:val=\"\"/></m:dPr><m:e><m:eqArr><m:e><m:r><m:t>x</m:t></m:r><m:r><m:t>y</m:t></m:r></m:e><m:e><m:r><m:t>z</m:t></m:r><m:r><m:t>w</m:t></m:r></m:e></m:eqArr></m:e></m:d></m:oMath>"),
        ("\\begin{cases}x\\end{cases}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:d><m:dPr><m:begChr m:val=\"{\"/><m:endChr m:val=\"\"/></m:dPr><m:e><m:eqArr><m:e><m:r><m:t>x</m:t></m:r></m:e></m:eqArr></m:e></m:d></m:oMath>"),
        ("\\begin{aligned} a &= b \\\\ c &= d \\end{aligned}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:eqArr><m:e><m:r><m:t>a</m:t></m:r><m:r><m:t>=</m:t></m:r><m:r><m:t>b</m:t></m:r></m:e><m:e><m:r><m:t>c</m:t></m:r><m:r><m:t>=</m:t></m:r><m:r><m:t>d</m:t></m:r></m:e></m:eqArr></m:oMath>"),
        ("\\begin{aligned} a \\\\ \\end{aligned}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:eqArr><m:e><m:r><m:t>a</m:t></m:r></m:e></m:eqArr></m:oMath>"),
        ("\\begin{array}{cc} 1 & 2 \\end{array}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:m><m:mr><m:e><m:r><m:t>{</m:t></m:r><m:r><m:t>c</m:t></m:r><m:r><m:t>c</m:t></m:r><m:r><m:t>}</m:t></m:r><m:r><m:t>1</m:t></m:r></m:e><m:e><m:r><m:t>2</m:t></m:r></m:e></m:mr></m:m></m:oMath>"),
        ("\\begin{array}{} x \\end{array}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:m><m:mr><m:e><m:r><m:t>{</m:t></m:r><m:r><m:t>}</m:t></m:r><m:r><m:t>x</m:t></m:r></m:e></m:mr></m:m></m:oMath>"),
        ("\\begin{gather*} a \\end{gather*}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:m><m:mr><m:e><m:r><m:t>a</m:t></m:r></m:e></m:mr></m:m></m:oMath>"),
        ("\\begin{matrix}1\\end{matrix}\\begin{matrix}2\\end{matrix}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:m><m:mr><m:e><m:r><m:t>1</m:t></m:r></m:e></m:mr></m:m><m:m><m:mr><m:e><m:r><m:t>2</m:t></m:r></m:e></m:mr></m:m></m:oMath>"),
        ("\\begin", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:m></m:m></m:oMath>"),
        ("\\begin{matrix}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:m></m:m></m:oMath>"),
        ("\\end{matrix}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>end</m:t></m:r><m:r><m:t>{</m:t></m:r><m:r><m:t>m</m:t></m:r><m:r><m:t>a</m:t></m:r><m:r><m:t>t</m:t></m:r><m:r><m:t>r</m:t></m:r><m:r><m:t>i</m:t></m:r><m:r><m:t>x</m:t></m:r><m:r><m:t>}</m:t></m:r></m:oMath>"),
        ("\\vec{a} \\cdot \\hat{b}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:acc><m:accPr><m:chr m:val=\"→\"/></m:accPr><m:e><m:r><m:t>a</m:t></m:r></m:e></m:acc><m:r><m:t>·</m:t></m:r><m:acc><m:accPr><m:chr m:val=\"^\"/></m:accPr><m:e><m:r><m:t>b</m:t></m:r></m:e></m:acc></m:oMath>"),
        ("\\text{hello} & \\mathrm{d}x \\mathbf{v} \\mathbb{R} \\mathcal{F} \\boldsymbol{\\alpha}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:rPr><m:nor/></m:rPr><m:t>hello</m:t></m:r><m:r><m:t>&amp;</m:t></m:r><m:r><m:rPr><m:i m:val=\"off\"/></m:rPr><m:t>d</m:t></m:r><m:r><m:t>x</m:t></m:r><m:r><m:rPr><m:b/></m:rPr><m:t>v</m:t></m:r><m:r><m:t>R</m:t></m:r><m:r><m:t>F</m:t></m:r><m:r><m:rPr><m:b/></m:rPr><m:t>\\alpha</m:t></m:r></m:oMath>"),
        ("\\text{a\\text{b}c}d", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:rPr><m:nor/></m:rPr><m:t>a\\text{b}c</m:t></m:r><m:r><m:t>d</m:t></m:r></m:oMath>"),
        ("\\text{}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:rPr><m:nor/></m:rPr><m:t></m:t></m:r></m:oMath>"),
        ("\\text{a}b", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:rPr><m:nor/></m:rPr><m:t>a</m:t></m:r><m:r><m:t>b</m:t></m:r></m:oMath>"),
        ("\\sin \\cos \\tan \\log_2 x \\lim_{x \\to 0} \\frac{1}{x}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>sin </m:t></m:r><m:r><m:t>cos </m:t></m:r><m:r><m:t>tan </m:t></m:r><m:sSub><m:e><m:r><m:t>log </m:t></m:r></m:e><m:sub><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sub></m:sSub><m:r><m:t>x</m:t></m:r><m:limLow><m:e><m:r><m:t>lim</m:t></m:r></m:e><m:lim><m:e><m:r><m:t>x</m:t></m:r><m:r><m:t>→</m:t></m:r><m:r><m:t>0</m:t></m:r></m:e></m:lim></m:limLow><m:f><m:num><m:e><m:r><m:t>1</m:t></m:r></m:e></m:num><m:den><m:e><m:r><m:t>x</m:t></m:r></m:e></m:den></m:f></m:oMath>"),
        ("\\lim x", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>lim </m:t></m:r><m:r><m:t>x</m:t></m:r></m:oMath>"),
        ("\\max_{n} \\min_{m}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:limLow><m:e><m:r><m:t>max</m:t></m:r></m:e><m:lim><m:e><m:r><m:t>n</m:t></m:r></m:e></m:lim></m:limLow><m:limLow><m:e><m:r><m:t>min</m:t></m:r></m:e><m:lim><m:e><m:r><m:t>m</m:t></m:r></m:e></m:lim></m:limLow></m:oMath>"),
        ("\\,\\; \\quad\\qquad\\!", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t> </m:t></m:r><m:r><m:t> </m:t></m:r><m:r><m:t>  </m:t></m:r><m:r><m:t>    </m:t></m:r></m:oMath>"),
        ("\\% \\_ \\& \\# 100\\%", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>%</m:t></m:r><m:r><m:t>_</m:t></m:r><m:r><m:t>&amp;</m:t></m:r><m:r><m:t>#</m:t></m:r><m:r><m:t>1</m:t></m:r><m:r><m:t>0</m:t></m:r><m:r><m:t>0</m:t></m:r><m:r><m:t>%</m:t></m:r></m:oMath>"),
        ("50\\% \\& 'quote' <tag> \"double\"", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>5</m:t></m:r><m:r><m:t>0</m:t></m:r><m:r><m:t>%</m:t></m:r><m:r><m:t>&amp;</m:t></m:r><m:r><m:t>'</m:t></m:r><m:r><m:t>q</m:t></m:r><m:r><m:t>u</m:t></m:r><m:r><m:t>o</m:t></m:r><m:r><m:t>t</m:t></m:r><m:r><m:t>e</m:t></m:r><m:r><m:t>'</m:t></m:r><m:r><m:t>&lt;</m:t></m:r><m:r><m:t>t</m:t></m:r><m:r><m:t>a</m:t></m:r><m:r><m:t>g</m:t></m:r><m:r><m:t>&gt;</m:t></m:r><m:r><m:t>\"</m:t></m:r><m:r><m:t>d</m:t></m:r><m:r><m:t>o</m:t></m:r><m:r><m:t>u</m:t></m:r><m:r><m:t>b</m:t></m:r><m:r><m:t>l</m:t></m:r><m:r><m:t>e</m:t></m:r><m:r><m:t>\"</m:t></m:r></m:oMath>"),
        ("x_i^2", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:sSubSup><m:e><m:r><m:t>x</m:t></m:r></m:e><m:sub><m:e><m:r><m:t>i</m:t></m:r></m:e></m:sub><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSubSup></m:oMath>"),
        ("{x_i}^2", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>{</m:t></m:r><m:sSub><m:e><m:r><m:t>x</m:t></m:r></m:e><m:sub><m:e><m:r><m:t>i</m:t></m:r></m:e></m:sub></m:sSub><m:sSup><m:e><m:r><m:t>}</m:t></m:r></m:e><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSup></m:oMath>"),
        ("a_b^c^d", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:sSup><m:e><m:sSubSup><m:e><m:r><m:t>a</m:t></m:r></m:e><m:sub><m:e><m:r><m:t>b</m:t></m:r></m:e></m:sub><m:sup><m:e><m:r><m:t>c</m:t></m:r></m:e></m:sup></m:sSubSup></m:e><m:sup><m:e><m:r><m:t>d</m:t></m:r></m:e></m:sup></m:sSup></m:oMath>"),
        ("^{2}x", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:sSup><m:e></m:e><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSup><m:r><m:t>x</m:t></m:r></m:oMath>"),
        ("x^{a^{b}}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:sSup><m:e><m:r><m:t>x</m:t></m:r></m:e><m:sup><m:e><m:sSup><m:e><m:r><m:t>a</m:t></m:r></m:e><m:sup><m:e><m:r><m:t>b</m:t></m:r></m:e></m:sup></m:sSup></m:e></m:sup></m:sSup></m:oMath>"),
        ("^_", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:sSup><m:e></m:e><m:sup><m:e><m:sSub><m:e></m:e><m:sub><m:e></m:e></m:sub></m:sSub></m:e></m:sup></m:sSup></m:oMath>"),
        ("a^", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:sSup><m:e><m:r><m:t>a</m:t></m:r></m:e><m:sup><m:e></m:e></m:sup></m:sSup></m:oMath>"),
        ("_x", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:sSub><m:e></m:e><m:sub><m:e><m:r><m:t>x</m:t></m:r></m:e></m:sub></m:sSub></m:oMath>"),
        ("$$x^2$$", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:sSup><m:e><m:r><m:t>x</m:t></m:r></m:e><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSup></m:oMath></m:oMathPara>"),
        ("$x^2$", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:sSup><m:e><m:r><m:t>x</m:t></m:r></m:e><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSup></m:oMath>"),
        ("$$", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:r><m:t></m:t></m:r></m:oMath></m:oMathPara>"),
        ("$", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t></m:t></m:r></m:oMath>"),
        ("x$$y", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>x</m:t></m:r><m:r><m:t>$</m:t></m:r><m:r><m:t>$</m:t></m:r><m:r><m:t>y</m:t></m:r></m:oMath>"),
        ("$$ x $$ and $ y $", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>$</m:t></m:r><m:r><m:t>x</m:t></m:r><m:r><m:t>$</m:t></m:r><m:r><m:t>$</m:t></m:r><m:r><m:t>a</m:t></m:r><m:r><m:t>n</m:t></m:r><m:r><m:t>d</m:t></m:r><m:r><m:t>$</m:t></m:r><m:r><m:t>y</m:t></m:r></m:oMath>"),
        ("\\sqrt[3]{x}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:rad><m:deg><m:e><m:r><m:t>3</m:t></m:r></m:e></m:deg><m:e><m:r><m:t>x</m:t></m:r></m:e></m:rad></m:oMath>"),
        ("\\sqrt[]{x}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/><m:e><m:r><m:t>x</m:t></m:r></m:e></m:rad></m:oMath>"),
        ("\\sqrt2", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/><m:e><m:r><m:t>2</m:t></m:r></m:e></m:rad></m:oMath>"),
        ("\\sqrt{x}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/><m:e><m:r><m:t>x</m:t></m:r></m:e></m:rad></m:oMath>"),
        ("\\frac12", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:f><m:num><m:e><m:r><m:t>1</m:t></m:r></m:e></m:num><m:den><m:e><m:r><m:t>2</m:t></m:r></m:e></m:den></m:f></m:oMath>"),
        ("\\frac{a}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:f><m:num><m:e><m:r><m:t>a</m:t></m:r></m:e></m:num><m:den><m:e></m:e></m:den></m:f></m:oMath>"),
        ("\\binom{n}{k}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:f><m:fPr><m:type m:val=\"noBar\"/></m:fPr><m:num><m:e><m:r><m:t>n</m:t></m:r></m:e></m:num><m:den><m:e><m:r><m:t>k</m:t></m:r></m:e></m:den></m:f></m:e></m:d></m:oMath>"),
        ("\\dfrac{1}{2}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:f><m:num><m:e><m:r><m:t>1</m:t></m:r></m:e></m:num><m:den><m:e><m:r><m:t>2</m:t></m:r></m:e></m:den></m:f></m:oMath>"),
        ("\\unknowncommand{x}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>unknowncommand</m:t></m:r><m:r><m:t>{</m:t></m:r><m:r><m:t>x</m:t></m:r><m:r><m:t>}</m:t></m:r></m:oMath>"),
        ("\\boxed{E=mc^2}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:borderBox><m:e><m:r><m:t>E</m:t></m:r><m:r><m:t>=</m:t></m:r><m:r><m:t>m</m:t></m:r><m:sSup><m:e><m:r><m:t>c</m:t></m:r></m:e><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSup></m:e></m:borderBox></m:oMath>"),
        ("\\boxed{}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:borderBox><m:e></m:e></m:borderBox></m:oMath>"),
        ("\\overline{AB}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:bar><m:barPr><m:pos m:val=\"top\"/></m:barPr><m:e><m:r><m:t>A</m:t></m:r><m:r><m:t>B</m:t></m:r></m:e></m:bar></m:oMath>"),
        ("\\bar{x} \\dot{y} \\ddot{z} \\tilde{w} \\check{v} \\acute{u} \\grave{t}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:acc><m:accPr><m:chr m:val=\"¯\"/></m:accPr><m:e><m:r><m:t>x</m:t></m:r></m:e></m:acc><m:acc><m:accPr><m:chr m:val=\"˙\"/></m:accPr><m:e><m:r><m:t>y</m:t></m:r></m:e></m:acc><m:acc><m:accPr><m:chr m:val=\"¨\"/></m:accPr><m:e><m:r><m:t>z</m:t></m:r></m:e></m:acc><m:acc><m:accPr><m:chr m:val=\"~\"/></m:accPr><m:e><m:r><m:t>w</m:t></m:r></m:e></m:acc><m:acc><m:accPr><m:chr m:val=\"ˇ\"/></m:accPr><m:e><m:r><m:t>v</m:t></m:r></m:e></m:acc><m:acc><m:accPr><m:chr m:val=\"´\"/></m:accPr><m:e><m:r><m:t>u</m:t></m:r></m:e></m:acc><m:acc><m:accPr><m:chr m:val=\"`\"/></m:accPr><m:e><m:r><m:t>t</m:t></m:r></m:e></m:acc></m:oMath>"),
        ("\\overline", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:bar><m:barPr><m:pos m:val=\"top\"/></m:barPr><m:e></m:e></m:bar></m:oMath>"),
        ("\\alpha\\beta\\gamma\\delta\\epsilon\\varepsilon\\zeta\\eta\\theta\\vartheta", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>α</m:t></m:r><m:r><m:t>β</m:t></m:r><m:r><m:t>γ</m:t></m:r><m:r><m:t>δ</m:t></m:r><m:r><m:t>ϵ</m:t></m:r><m:r><m:t>ε</m:t></m:r><m:r><m:t>ζ</m:t></m:r><m:r><m:t>η</m:t></m:r><m:r><m:t>θ</m:t></m:r><m:r><m:t>ϑ</m:t></m:r></m:oMath>"),
        ("\\iint_D \\oint_C f", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:nary><m:naryPr><m:chr m:val=\"∬\"/><m:limLoc m:val=\"subSup\"/></m:naryPr><m:sub><m:e><m:r><m:t>D</m:t></m:r></m:e></m:sub><m:sup/><m:e/></m:nary><m:nary><m:naryPr><m:chr m:val=\"∮\"/><m:limLoc m:val=\"subSup\"/></m:naryPr><m:sub><m:e><m:r><m:t>C</m:t></m:r></m:e></m:sub><m:sup/><m:e/></m:nary><m:r><m:t>f</m:t></m:r></m:oMath>"),
        ("\\bigcup_\\bigcap", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:nary><m:naryPr><m:chr m:val=\"⋃\"/><m:limLoc m:val=\"subSup\"/></m:naryPr><m:sub><m:e><m:nary><m:naryPr><m:chr m:val=\"⋂\"/><m:limLoc m:val=\"subSup\"/></m:naryPr><m:sub/><m:sup/><m:e/></m:nary></m:e></m:sub><m:sup/><m:e/></m:nary></m:oMath>"),
        ("\\sum", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:nary><m:naryPr><m:chr m:val=\"∑\"/><m:limLoc m:val=\"undOvr\"/></m:naryPr><m:sub/><m:sup/><m:e/></m:nary></m:oMath>"),
        ("\\sum^{a}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:nary><m:naryPr><m:chr m:val=\"∑\"/><m:limLoc m:val=\"undOvr\"/></m:naryPr><m:sub/><m:sup><m:e><m:r><m:t>a</m:t></m:r></m:e></m:sup><m:e/></m:nary></m:oMath>"),
        ("\\prod\\limits_{i}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:nary><m:naryPr><m:chr m:val=\"∏\"/><m:limLoc m:val=\"subSup\"/></m:naryPr><m:sub/><m:sup/><m:e/></m:nary><m:sSub><m:e><m:r><m:t>limits</m:t></m:r></m:e><m:sub><m:e><m:r><m:t>i</m:t></m:r></m:e></m:sub></m:sSub></m:oMath>"),
        ("a \\ne b \\le c \\ge d \\approx e \\equiv f \\sim g \\propto h", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>a</m:t></m:r><m:r><m:t>≠</m:t></m:r><m:r><m:t>b</m:t></m:r><m:r><m:t>≤</m:t></m:r><m:r><m:t>c</m:t></m:r><m:r><m:t>≥</m:t></m:r><m:r><m:t>d</m:t></m:r><m:r><m:t>≈</m:t></m:r><m:r><m:t>e</m:t></m:r><m:r><m:t>≡</m:t></m:r><m:r><m:t>f</m:t></m:r><m:r><m:t>∼</m:t></m:r><m:r><m:t>g</m:t></m:r><m:r><m:t>∝</m:t></m:r><m:r><m:t>h</m:t></m:r></m:oMath>"),
        ("f(x) = \\begin{cases} 1 & x > 0 \\\\ 0 & x \\le 0 \\end{cases}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>f</m:t></m:r><m:r><m:t>(</m:t></m:r><m:r><m:t>x</m:t></m:r><m:r><m:t>)</m:t></m:r><m:r><m:t>=</m:t></m:r><m:d><m:dPr><m:begChr m:val=\"{\"/><m:endChr m:val=\"\"/></m:dPr><m:e><m:eqArr><m:e><m:r><m:t>1</m:t></m:r><m:r><m:t>x</m:t></m:r><m:r><m:t>&gt;</m:t></m:r><m:r><m:t>0</m:t></m:r></m:e><m:e><m:r><m:t>0</m:t></m:r><m:r><m:t>x</m:t></m:r><m:r><m:t>≤</m:t></m:r><m:r><m:t>0</m:t></m:r></m:e></m:eqArr></m:e></m:d></m:oMath>"),
        ("\\int_a^b f(x)\\,dx = F(b) - F(a)", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:nary><m:naryPr><m:chr m:val=\"∫\"/><m:limLoc m:val=\"subSup\"/></m:naryPr><m:sub><m:e><m:r><m:t>a</m:t></m:r></m:e></m:sub><m:sup><m:e><m:r><m:t>b</m:t></m:r></m:e></m:sup><m:e/></m:nary><m:r><m:t>f</m:t></m:r><m:r><m:t>(</m:t></m:r><m:r><m:t>x</m:t></m:r><m:r><m:t>)</m:t></m:r><m:r><m:t> </m:t></m:r><m:r><m:t>d</m:t></m:r><m:r><m:t>x</m:t></m:r><m:r><m:t>=</m:t></m:r><m:r><m:t>F</m:t></m:r><m:r><m:t>(</m:t></m:r><m:r><m:t>b</m:t></m:r><m:r><m:t>)</m:t></m:r><m:r><m:t>-</m:t></m:r><m:r><m:t>F</m:t></m:r><m:r><m:t>(</m:t></m:r><m:r><m:t>a</m:t></m:r><m:r><m:t>)</m:t></m:r></m:oMath>"),
        ("\\begin{pmatrix} a \\end{pmatrix}_i^2", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:sSubSup><m:e><m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:m><m:mr><m:e><m:r><m:t>a</m:t></m:r></m:e></m:mr></m:m></m:e></m:d></m:e><m:sub><m:e><m:r><m:t>i</m:t></m:r></m:e></m:sub><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSubSup></m:oMath>"),
        ("A \\cap B \\cup C \\setminus D", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>A</m:t></m:r><m:r><m:t>∩</m:t></m:r><m:r><m:t>B</m:t></m:r><m:r><m:t>∪</m:t></m:r><m:r><m:t>C</m:t></m:r><m:r><m:t>∖</m:t></m:r><m:r><m:t>D</m:t></m:r></m:oMath>"),
        ("\\{ a \\} \\langle u | v \\rangle", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>{</m:t></m:r><m:r><m:t>a</m:t></m:r><m:r><m:t>}</m:t></m:r><m:r><m:t>⟨</m:t></m:r><m:r><m:t>u</m:t></m:r><m:r><m:t>|</m:t></m:r><m:r><m:t>v</m:t></m:r><m:r><m:t>⟩</m:t></m:r></m:oMath>"),
        ("\\forall x \\exists y : x \\in S", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>∀</m:t></m:r><m:r><m:t>x</m:t></m:r><m:r><m:t>∃</m:t></m:r><m:r><m:t>y</m:t></m:r><m:r><m:t>:</m:t></m:r><m:r><m:t>x</m:t></m:r><m:r><m:t>∈</m:t></m:r><m:r><m:t>S</m:t></m:r></m:oMath>"),
        ("a\\b", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>a</m:t></m:r><m:r><m:t>b</m:t></m:r></m:oMath>"),
        ("a b", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>a</m:t></m:r><m:r><m:t>b</m:t></m:r></m:oMath>"),
        ("   ", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t></m:t></m:r></m:oMath>"),
        ("", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t></m:t></m:r></m:oMath>"),
        ("   \\sum   ", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:nary><m:naryPr><m:chr m:val=\"∑\"/><m:limLoc m:val=\"undOvr\"/></m:naryPr><m:sub/><m:sup/><m:e/></m:nary></m:oMath>"),
        ("\t\\alpha\n\\beta ", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>α</m:t></m:r><m:r><m:t>β</m:t></m:r></m:oMath>"),
        ("a\u{001c}b", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>a</m:t></m:r><m:r><m:t>b</m:t></m:r></m:oMath>"),
        ("a
b", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>a</m:t></m:r><m:r><m:t>b</m:t></m:r></m:oMath>"),
        ("a b", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>a</m:t></m:r><m:r><m:t>b</m:t></m:r></m:oMath>"),
        ("\\mathrm{d}x", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:rPr><m:i m:val=\"off\"/></m:rPr><m:t>d</m:t></m:r><m:r><m:t>x</m:t></m:r></m:oMath>"),
        ("\\cal{F}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>cal</m:t></m:r><m:r><m:t>{</m:t></m:r><m:r><m:t>F</m:t></m:r><m:r><m:t>}</m:t></m:r></m:oMath>"),
        ("\\mathcal{F}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>F</m:t></m:r></m:oMath>"),
        ("\\LaTeX \\TeX \\TeXture", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>LaTeX</m:t></m:r><m:r><m:t>TeX</m:t></m:r><m:r><m:t>TeXture</m:t></m:r></m:oMath>"),
        ("\\operatorname{sn} x", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>operatorname</m:t></m:r><m:r><m:t>{</m:t></m:r><m:r><m:t>s</m:t></m:r><m:r><m:t>n</m:t></m:r><m:r><m:t>}</m:t></m:r><m:r><m:t>x</m:t></m:r></m:oMath>"),
        ("&", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>&amp;</m:t></m:r></m:oMath>"),
        ("&&", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>&amp;</m:t></m:r><m:r><m:t>&amp;</m:t></m:r></m:oMath>"),
        ("\\\\", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>\\</m:t></m:r></m:oMath>"),
        ("Ω x", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>Ω</m:t></m:r><m:r><m:t>x</m:t></m:r></m:oMath>"),
        ("\\alpha_{1}^{n}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:sSubSup><m:e><m:r><m:t>α</m:t></m:r></m:e><m:sub><m:e><m:r><m:t>1</m:t></m:r></m:e></m:sub><m:sup><m:e><m:r><m:t>n</m:t></m:r></m:e></m:sup></m:sSubSup></m:oMath>"),
        ("{}^1_2H", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>{</m:t></m:r><m:sSub><m:e><m:sSup><m:e><m:r><m:t>}</m:t></m:r></m:e><m:sup><m:e><m:r><m:t>1</m:t></m:r></m:e></m:sup></m:sSup></m:e><m:sub><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sub></m:sSub><m:r><m:t>H</m:t></m:r></m:oMath>"),
        ("\\begin{cases} a & b \\\\ \\end{cases}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:d><m:dPr><m:begChr m:val=\"{\"/><m:endChr m:val=\"\"/></m:dPr><m:e><m:eqArr><m:e><m:r><m:t>a</m:t></m:r><m:r><m:t>b</m:t></m:r></m:e></m:eqArr></m:e></m:d></m:oMath>"),
        ("\\frac{\\frac{a}{b}}{c}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:f><m:num><m:e><m:f><m:num><m:e><m:r><m:t>a</m:t></m:r></m:e></m:num><m:den><m:e><m:r><m:t>b</m:t></m:r></m:e></m:den></m:f></m:e></m:num><m:den><m:e><m:r><m:t>c</m:t></m:r></m:e></m:den></m:f></m:oMath>"),
        ("\\left( \\begin{matrix} 1 \\\\ 2 \\end{matrix} \\right)", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:m><m:mr><m:e><m:r><m:t>1</m:t></m:r></m:e></m:mr><m:mr><m:e><m:r><m:t>2</m:t></m:r><m:r><m:t>end</m:t></m:r><m:r><m:t>{</m:t></m:r><m:r><m:t>m</m:t></m:r><m:r><m:t>a</m:t></m:r><m:r><m:t>t</m:t></m:r><m:r><m:t>r</m:t></m:r><m:r><m:t>i</m:t></m:r><m:r><m:t>x</m:t></m:r><m:r><m:t>}</m:t></m:r></m:e></m:mr></m:m></m:e></m:d></m:oMath>"),
        ("\\text{spin-up } \\uparrow \\text{ spin-down } \\downarrow", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:rPr><m:nor/></m:rPr><m:t>spin-up </m:t></m:r><m:r><m:t>↑</m:t></m:r><m:r><m:rPr><m:nor/></m:rPr><m:t> spin-down </m:t></m:r><m:r><m:t>↓</m:t></m:r></m:oMath>"),
        ("\\overline{\\overline{A}}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:bar><m:barPr><m:pos m:val=\"top\"/></m:barPr><m:e><m:bar><m:barPr><m:pos m:val=\"top\"/></m:barPr><m:e><m:r><m:t>A</m:t></m:r></m:e></m:bar></m:e></m:bar></m:oMath>"),
        ("\\sqrt{\\sqrt{x}}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/><m:e><m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/><m:e><m:r><m:t>x</m:t></m:r></m:e></m:rad></m:e></m:rad></m:oMath>"),
        ("\\sum_{i=1}^{\\infty} \\frac{1}{i^2} = \\frac{\\pi^2}{6}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:nary><m:naryPr><m:chr m:val=\"∑\"/><m:limLoc m:val=\"undOvr\"/></m:naryPr><m:sub><m:e><m:r><m:t>i</m:t></m:r><m:r><m:t>=</m:t></m:r><m:r><m:t>1</m:t></m:r></m:e></m:sub><m:sup><m:e><m:r><m:t>∞</m:t></m:r></m:e></m:sup><m:e/></m:nary><m:f><m:num><m:e><m:r><m:t>1</m:t></m:r></m:e></m:num><m:den><m:e><m:sSup><m:e><m:r><m:t>i</m:t></m:r></m:e><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSup></m:e></m:den></m:f><m:r><m:t>=</m:t></m:r><m:f><m:num><m:e><m:sSup><m:e><m:r><m:t>π</m:t></m:r></m:e><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSup></m:e></m:num><m:den><m:e><m:r><m:t>6</m:t></m:r></m:e></m:den></m:f></m:oMath>"),
        ("x'_i", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>x</m:t></m:r><m:sSub><m:e><m:r><m:t>'</m:t></m:r></m:e><m:sub><m:e><m:r><m:t>i</m:t></m:r></m:e></m:sub></m:sSub></m:oMath>"),
        ("'", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>'</m:t></m:r></m:oMath>"),
        ("\\overrightarrow{AB}", "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:r><m:t>overrightarrow</m:t></m:r><m:r><m:t>{</m:t></m:r><m:r><m:t>A</m:t></m:r><m:r><m:t>B</m:t></m:r><m:r><m:t>}</m:t></m:r></m:oMath>"),
        ];
        assert_matches_python(cases, false);
    }

    /// 块级公式（is_block=True，外层 <m:oMathPara>）逐字符对照表。
    #[test]
    fn omml_block_matches_python_character_by_character() {
        let cases: &[(&str, &str)] = &[
        ("E = mc^2", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:r><m:t>E</m:t></m:r><m:r><m:t>=</m:t></m:r><m:r><m:t>m</m:t></m:r><m:sSup><m:e><m:r><m:t>c</m:t></m:r></m:e><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSup></m:oMath></m:oMathPara>"),
        ("\\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:f><m:num><m:e><m:r><m:t>-</m:t></m:r><m:r><m:t>b</m:t></m:r><m:r><m:t>±</m:t></m:r><m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/><m:e><m:sSup><m:e><m:r><m:t>b</m:t></m:r></m:e><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSup><m:r><m:t>-</m:t></m:r><m:r><m:t>4</m:t></m:r><m:r><m:t>a</m:t></m:r><m:r><m:t>c</m:t></m:r></m:e></m:rad></m:e></m:num><m:den><m:e><m:r><m:t>2</m:t></m:r><m:r><m:t>a</m:t></m:r></m:e></m:den></m:f></m:oMath></m:oMathPara>"),
        ("\\sum_{i=1}^{n} i^2", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:nary><m:naryPr><m:chr m:val=\"∑\"/><m:limLoc m:val=\"undOvr\"/></m:naryPr><m:sub><m:e><m:r><m:t>i</m:t></m:r><m:r><m:t>=</m:t></m:r><m:r><m:t>1</m:t></m:r></m:e></m:sub><m:sup><m:e><m:r><m:t>n</m:t></m:r></m:e></m:sup><m:e/></m:nary><m:sSup><m:e><m:r><m:t>i</m:t></m:r></m:e><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSup></m:oMath></m:oMathPara>"),
        ("\\int_0^1 x^2 dx", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:nary><m:naryPr><m:chr m:val=\"∫\"/><m:limLoc m:val=\"subSup\"/></m:naryPr><m:sub><m:e><m:r><m:t>0</m:t></m:r></m:e></m:sub><m:sup><m:e><m:r><m:t>1</m:t></m:r></m:e></m:sup><m:e/></m:nary><m:sSup><m:e><m:r><m:t>x</m:t></m:r></m:e><m:sup><m:e><m:r><m:t>2</m:t></m:r></m:e></m:sup></m:sSup><m:r><m:t>d</m:t></m:r><m:r><m:t>x</m:t></m:r></m:oMath></m:oMathPara>"),
        ("\\left( \\frac{a}{b} \\right)", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:f><m:num><m:e><m:r><m:t>a</m:t></m:r></m:e></m:num><m:den><m:e><m:r><m:t>b</m:t></m:r></m:e></m:den></m:f></m:e></m:d></m:oMath></m:oMathPara>"),
        ("\\left\\{ x \\right}", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:d><m:dPr><m:begChr m:val=\"\\{\"/><m:endChr m:val=\"}\"/></m:dPr><m:e><m:r><m:t>x</m:t></m:r></m:e></m:d></m:oMath></m:oMathPara>"),
        ("\\left[ 0, 1 \\right]", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:d><m:dPr><m:begChr m:val=\"[\"/><m:endChr m:val=\"]\"/></m:dPr><m:e><m:r><m:t>0</m:t></m:r><m:r><m:t>,</m:t></m:r><m:r><m:t>1</m:t></m:r></m:e></m:d></m:oMath></m:oMathPara>"),
        ("\\left. \\frac{d}{dx} \\right|_{x=0}", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:sSub><m:e><m:d><m:dPr><m:begChr m:val=\"\"/><m:endChr m:val=\"|\"/></m:dPr><m:e><m:f><m:num><m:e><m:r><m:t>d</m:t></m:r></m:e></m:num><m:den><m:e><m:r><m:t>d</m:t></m:r><m:r><m:t>x</m:t></m:r></m:e></m:den></m:f></m:e></m:d></m:e><m:sub><m:e><m:r><m:t>x</m:t></m:r><m:r><m:t>=</m:t></m:r><m:r><m:t>0</m:t></m:r></m:e></m:sub></m:sSub></m:oMath></m:oMathPara>"),
        ("\\left\\| x \\right\\|", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:d><m:dPr><m:begChr m:val=\"\\|\"/><m:endChr m:val=\"\\|\"/></m:dPr><m:e><m:r><m:t>x</m:t></m:r></m:e></m:d></m:oMath></m:oMathPara>"),
        ("\\left\\{ a \\right\\}", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:d><m:dPr><m:begChr m:val=\"\\{\"/><m:endChr m:val=\"\\}\"/></m:dPr><m:e><m:r><m:t>a</m:t></m:r></m:e></m:d></m:oMath></m:oMathPara>"),
        ("\\left(a\\right)", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:r><m:t>a</m:t></m:r></m:e></m:d></m:oMath></m:oMathPara>"),
        ("\\left(x", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:r><m:t>x</m:t></m:r></m:e></m:d></m:oMath></m:oMathPara>"),
        ("\\right)", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:r><m:t>right</m:t></m:r><m:r><m:t>)</m:t></m:r></m:oMath></m:oMathPara>"),
        ("\\left(\\left(a\\right)\\right)", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:r><m:t>a</m:t></m:r></m:e></m:d></m:e></m:d></m:oMath></m:oMathPara>"),
        ("\\begin{pmatrix} 1 & 2 \\\\ 3 & 4 \\end{pmatrix}", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\")\"/></m:dPr><m:e><m:m><m:mr><m:e><m:r><m:t>1</m:t></m:r></m:e><m:e><m:r><m:t>2</m:t></m:r></m:e></m:mr><m:mr><m:e><m:r><m:t>3</m:t></m:r></m:e><m:e><m:r><m:t>4</m:t></m:r></m:e></m:mr></m:m></m:e></m:d></m:oMath></m:oMathPara>"),
        ("\\begin{bmatrix} a \\end{bmatrix}", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:d><m:dPr><m:begChr m:val=\"[\"/><m:endChr m:val=\"]\"/></m:dPr><m:e><m:m><m:mr><m:e><m:r><m:t>a</m:t></m:r></m:e></m:mr></m:m></m:e></m:d></m:oMath></m:oMathPara>"),
        ("\\begin{vmatrix} 1 & 0 \\\\ 0 & 1 \\end{vmatrix}", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:d><m:dPr><m:begChr m:val=\"|\"/><m:endChr m:val=\"|\"/></m:dPr><m:e><m:m><m:mr><m:e><m:r><m:t>1</m:t></m:r></m:e><m:e><m:r><m:t>0</m:t></m:r></m:e></m:mr><m:mr><m:e><m:r><m:t>0</m:t></m:r></m:e><m:e><m:r><m:t>1</m:t></m:r></m:e></m:mr></m:m></m:e></m:d></m:oMath></m:oMathPara>"),
        ("\\begin{matrix} 1 & 2 \\\\ 3 & 4 \\end{matrix}", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:m><m:mr><m:e><m:r><m:t>1</m:t></m:r></m:e><m:e><m:r><m:t>2</m:t></m:r></m:e></m:mr><m:mr><m:e><m:r><m:t>3</m:t></m:r></m:e><m:e><m:r><m:t>4</m:t></m:r></m:e></m:mr></m:m></m:oMath></m:oMathPara>"),
        ("\\begin{cases} x & y \\\\ z & w \\end{cases}", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:d><m:dPr><m:begChr m:val=\"{\"/><m:endChr m:val=\"\"/></m:dPr><m:e><m:eqArr><m:e><m:r><m:t>x</m:t></m:r><m:r><m:t>y</m:t></m:r></m:e><m:e><m:r><m:t>z</m:t></m:r><m:r><m:t>w</m:t></m:r></m:e></m:eqArr></m:e></m:d></m:oMath></m:oMathPara>"),
        ("\\begin{cases}x\\end{cases}", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:d><m:dPr><m:begChr m:val=\"{\"/><m:endChr m:val=\"\"/></m:dPr><m:e><m:eqArr><m:e><m:r><m:t>x</m:t></m:r></m:e></m:eqArr></m:e></m:d></m:oMath></m:oMathPara>"),
        ("\\begin{aligned} a &= b \\\\ c &= d \\end{aligned}", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:eqArr><m:e><m:r><m:t>a</m:t></m:r><m:r><m:t>=</m:t></m:r><m:r><m:t>b</m:t></m:r></m:e><m:e><m:r><m:t>c</m:t></m:r><m:r><m:t>=</m:t></m:r><m:r><m:t>d</m:t></m:r></m:e></m:eqArr></m:oMath></m:oMathPara>"),
        ("\\begin{aligned} a \\\\ \\end{aligned}", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:eqArr><m:e><m:r><m:t>a</m:t></m:r></m:e></m:eqArr></m:oMath></m:oMathPara>"),
        ("\\begin{array}{cc} 1 & 2 \\end{array}", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:m><m:mr><m:e><m:r><m:t>{</m:t></m:r><m:r><m:t>c</m:t></m:r><m:r><m:t>c</m:t></m:r><m:r><m:t>}</m:t></m:r><m:r><m:t>1</m:t></m:r></m:e><m:e><m:r><m:t>2</m:t></m:r></m:e></m:mr></m:m></m:oMath></m:oMathPara>"),
        ("\\begin{array}{} x \\end{array}", "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><m:oMath><m:m><m:mr><m:e><m:r><m:t>{</m:t></m:r><m:r><m:t>}</m:t></m:r><m:r><m:t>x</m:t></m:r></m:e></m:mr></m:m></m:oMath></m:oMathPara>"),
        ];
        assert_matches_python(cases, true);
    }

    fn assert_matches_python(cases: &[(&str, &str)], is_block: bool) {
        let mut diffs: Vec<String> = Vec::new();
        for (input, expected) in cases {
            let got = latex_to_omml(input, is_block);
            if got != *expected {
                diffs.push(format!(
                    "input={}\n  python={}\n  rust  ={}",
                    input, expected, got
                ));
            }
        }
        assert!(
            diffs.is_empty(),
            "{} of {} OMML goldens differ (is_block={}):\n{}",
            diffs.len(),
            cases.len(),
            is_block,
            diffs.join("\n--\n")
        );
    }

    /// 结构自检：标签必须配平。
    ///
    /// Python 的输出天然良构；此前 Rust 把 <m:nary> 拼出 "</m:nary<m:e/>"，
    /// 又把 <m:d> 的闭合标签写到属性前面，这个检查可以兜住这一类回归。
    #[test]
    fn omml_goldens_have_balanced_tags() {
        let probes: &[&str] = &[
            "E = mc^2",
            "\\sum_{i=1}^{n} i^2",
            "\\left( \\frac{a}{b} \\right)",
            "\\left. \\frac{d}{dx} \\right|_{x=0}",
            "\\begin{pmatrix} 1 & 2 \\\\ 3 & 4 \\end{pmatrix}",
            "\\begin{cases} x & y \\end{cases}",
            "\\left\\{ x \\right}",
            "\\int_0^1 x^2 dx",
            "x_i^2",
            "\\binom{n}{k}",
            "\\sqrt[3]{x}",
            "\\text{hello} \\mathbb{R}",
            "\\lim_{x \\to 0} \\frac{1}{x}",
            "\\boxed{E=mc^2}",
            "\\overline{AB}",
        ];
        let tags: &[&str] = &[
            "m:r", "m:t", "m:e", "m:f", "m:num", "m:den", "m:rad", "m:deg", "m:sub", "m:sup",
            "m:nary", "m:d", "m:m", "m:mr", "m:acc", "m:bar", "m:eqArr", "m:borderBox",
            "m:limLow", "m:lim", "m:sSup", "m:sSub", "m:sSubSup",
        ];
        for probe in probes {
            for is_block in [false, true] {
                let xml = latex_to_omml(probe, is_block);
                for tag in tags {
                    let open = xml.matches(&format!("<{}>", tag)).count();
                    let close = xml.matches(&format!("</{}>", tag)).count();
                    assert_eq!(
                        open, close,
                        "unbalanced <{}> in {} (is_block={}): {}",
                        tag, probe, is_block, xml
                    );
                }
            }
        }
    }

    /// saxutils.escape 只转义 & < >：引号必须原样出现。
    #[test]
    fn escape_xml_matches_saxutils() {
        assert_eq!(escape_xml("&<>"), "&amp;&lt;&gt;");
        assert_eq!(escape_xml("&\"'"), "&amp;\"'");
        assert_eq!(escape_xml("a&b"), "a&amp;b");
        let xml = latex_to_omml("50\\% \\& 'quote' <tag> \"double\"", false);
        assert!(!xml.contains("&quot;"), "{}", xml);
        assert!(!xml.contains("&apos;"), "{}", xml);
        assert!(xml.contains("<m:t>\"</m:t>"), "{}", xml);
        assert!(xml.contains("<m:t>'</m:t>"), "{}", xml);
    }

    /// Python 的 isspace/strip 比 Rust 多认 \x1c-\x1f。
    #[test]
    fn python_whitespace_semantics() {
        assert!(py_isspace('\u{1c}'));
        assert!(py_isspace('\u{1f}'));
        assert!(py_isspace('\u{a0}'));
        assert!(py_isspace('\t'));
        assert!(!py_isspace('\u{7f}'));
        assert!(!py_isspace('x'));
        assert_eq!(py_trim("\u{1c} x \u{1f}"), "x");
        assert_eq!(py_trim("  \u{b}\u{c}\r\n x \t "), "x");
        // \x1c 在 Python 里就是普通空白，两边必须给出同一棵树
        assert_eq!(
            latex_to_omml("a\u{1c}b", false),
            latex_to_omml("a b", false)
        );
    }

    /// 只剩定界符时 Python 切片退化为空串，Rust 不能 panic。
    #[test]
    fn dollar_only_inputs_never_panic() {
        for input in ["$", "$$", "$$$", "$$$$", "x$$y", "$ $", "$a$b$", "$\\frac{1}{2}$"] {
            let _ = latex_to_omml(input, false);
            let _ = latex_to_omml(input, true);
        }
    }

    /// \begin 之后必须从当前位置找 \end{env}，否则连排环境会被切错。
    #[test]
    fn consecutive_environments_split_at_their_own_end() {
        let xml = latex_to_omml(
            "\\begin{matrix}1\\end{matrix}\\begin{matrix}2\\end{matrix}",
            false,
        );
        assert_eq!(xml.matches("<m:mr>").count(), 2, "{}", xml);
        assert_eq!(xml.matches("<m:m>").count(), 2, "{}", xml);
        assert!(xml.contains("<m:t>1</m:t>"), "{}", xml);
        assert!(xml.contains("<m:t>2</m:t>"), "{}", xml);
    }

    /// 定界符属性必须嵌在 <m:d> 内部（曾经提前闭合）。
    #[test]
    fn delimiter_properties_stay_inside_the_delimiter_element() {
        let xml = latex_to_omml("\\left( a \\right)", false);
        assert!(xml.contains("<m:d><m:dPr>"), "{}", xml);
        assert!(xml.contains("</m:dPr><m:e>"), "{}", xml);
        assert!(!xml.contains("</m:d><m:e>"), "{}", xml);
    }

    /// n 元运算符必须是 <m:nary><m:naryPr>…</m:naryPr>sub sup <m:e/></m:nary>。
    #[test]
    fn nary_emits_python_shape() {
        let xml = latex_to_omml("\\sum_{i=1}^{n}", false);
        assert!(
            xml.contains(
                "<m:nary><m:naryPr><m:chr m:val=\"\u{2211}\"/><m:limLoc m:val=\"undOvr\"/></m:naryPr>"
            ),
            "{}",
            xml
        );
        assert!(xml.contains("</m:sup><m:e/></m:nary>"), "{}", xml);
        assert!(!xml.contains("</m:nary<m:"), "{}", xml);
        let integral = latex_to_omml("\\int_a^b f", false);
        assert!(integral.contains("undOvr") == false, "{}", integral);
        assert!(integral.contains("subSup"), "{}", integral);
    }

    /// 字体命令集合与 Python 完全一致：有 \mathcal，没有 \cal。
    #[test]
    fn font_command_set_matches_python() {
        let mathcal = latex_to_omml("\\mathcal{F}", false);
        assert!(mathcal.contains("<m:r><m:t>F</m:t></m:r>"), "{}", mathcal);
        assert!(!mathcal.contains("rPr"), "{}", mathcal);
        let cal = latex_to_omml("\\cal{F}", false);
        assert!(cal.contains("<m:t>cal</m:t>"), "{}", cal);
        assert!(cal.contains("<m:t>{</m:t>"), "{}", cal);
    }
    // ------------------------------------------------- Item 1: call-stack safety
    //
    // `_parse_latex_to_omml_inner` used to consume one *call frame* per nesting
    // level.  The threads that reach this code get 1 MiB (the tao/wry overlay and
    // window threads) or 2 MiB (`server.rs` spawns `stack_size(2 << 20)`), so a
    // few thousand nested `\frac{…}{…}` in an ordinary `.md` was a hard abort, not
    // a recoverable error.  The parse now keeps one heap `Frame` per level, which
    // the call stack cannot see — so the regression these tests guard against is a
    // crash rather than a failed assertion, and it would still pass by luck on the
    // 8 MiB main thread.  Every input therefore runs on a deliberately tiny
    // 64 KiB thread: ~150 old frames fit there, and these go thousands deep.

    /// `(open, close, per-level tag)`; `open.repeat(n) + "x" + close.repeat(n)`
    /// nests exactly `n` groups, each of which the old parser descended into, and
    /// must come back as exactly `n` copies of `per-level tag`.
    const NESTING_SHAPES: &[(&str, &str, &str)] = &[
        (r"\frac{", r"}{1}", "<m:f>"),
        (r"\dfrac{", r"}{1}", "<m:f>"),
        (r"\binom{", r"}{1}", "<m:f>"),
        (r"\sqrt{", "}", "<m:rad>"),
        (r"\sqrt[3]{", "}", "<m:rad>"),
        (r"\overline{", "}", "<m:bar>"),
        // `\bar` is *not* an `<m:bar>`: Python's `_ACCENTS` (latex2omml.py:74)
        // already binds `\bar` to `¯`, and the accent branch (latex2omml.py:328)
        // runs before the `if t in (r'\overline', r'\bar')` branch
        // (latex2omml.py:343), which is therefore dead for `\bar`.  Measured
        // depth-500 output: `<m:bar>`=0, `<m:acc>`=500
        // (scratch/rust_parity/latex_probe_shapes.txt).  `<m:bar>` stays
        // covered by the `\overline{` row above.
        (r"\bar{", "}", "<m:acc>"),
        (r"\vec{", "}", "<m:acc>"),
        (r"\boxed{", "}", "<m:borderBox>"),
        (r"\lim_{", "}", "<m:limLow>"),
        (r"\sum_{", "}", "<m:nary>"),
        (r"\int^{", "}", "<m:nary>"),
        (r"x^{", "}", "<m:sSup>"),
        (r"x_{", "}", "<m:sSub>"),
        (r"\left(", r"\right)", "<m:d>"),
        (r"\begin{matrix}", r"\end{matrix}", "<m:m>"),
    ];

    fn nested(open: &str, close: &str, depth: usize) -> String {
        let mut s = String::with_capacity((open.len() + close.len()) * depth + 1);
        for _ in 0..depth {
            s.push_str(open);
        }
        s.push('x');
        for _ in 0..depth {
            s.push_str(close);
        }
        s
    }

    /// `latex_to_omml` on a 64 KiB stack; the `expect` messages double as the
    /// diagnosis when a regression turns this into a stack overflow.
    fn renders_on_64_kib_stack(src: String) -> String {
        std::thread::scope(|scope| {
            std::thread::Builder::new()
                .stack_size(64 << 10)
                .spawn_scoped(scope, move || latex_to_omml(&src, false))
                .expect("a 64 KiB thread must be spawnable")
                .join()
                .expect("64 KiB stack: the parser is using the call stack again")
        })
    }

    /// Deep nesting must neither overflow the stack nor quietly drop levels.
    #[test]
    fn deep_nesting_neither_overflows_nor_truncates() {
        for depth in [500usize, 2_000, 6_000] {
            for (open, close, tag) in NESTING_SHAPES {
                let out = renders_on_64_kib_stack(nested(open, close, depth));
                assert!(
                    out.starts_with("<m:oMath "),
                    "shape {:?} at depth {} did not render: {:?}",
                    open,
                    depth,
                    &out[..out.len().min(60)]
                );
                // A silent cut-off would pass a "did not crash" test while
                // deleting the user's formula, so every level has to be counted.
                assert_eq!(
                    out.matches(*tag).count(),
                    depth,
                    "{} at depth {}: expected one {:?} per nesting level",
                    open,
                    depth,
                    tag
                );
                assert!(
                    out.contains("<m:t>x</m:t>"),
                    "{} at depth {}: the innermost atom is gone",
                    open,
                    depth
                );
            }
        }
    }

    /// The reported crash shape specifically, across the measured CPython
    /// boundary (993 accepted / 994 `RecursionError`, see the note on
    /// `parse_latex_to_omml_inner`): the Rust port renders every depth.
    #[test]
    fn deep_frac_chain_renders_every_level() {
        for depth in [993usize, 994, 2_000, 8_000] {
            let out = renders_on_64_kib_stack(nested(r"\frac{", r"}{1}", depth));
            assert_eq!(
                out.matches("<m:f>").count(),
                depth,
                "depth {} must emit exactly one <m:f> per level",
                depth
            );
            assert_eq!(out.matches("<m:t>x</m:t>").count(), 1);
            assert_eq!(out.matches("<m:num>").count(), depth);
            assert_eq!(out.matches("<m:den>").count(), depth);
        }
    }

    /// Ordinary formulas must render byte-identically on that same tiny stack —
    /// the frame stack is not merely crash-safe, it is the same parser.
    #[test]
    fn ordinary_formulas_render_identically_on_a_tiny_stack() {
        for probe in DIFFERENTIAL_PROBES {
            let big = latex_to_omml(probe, false);
            let small = renders_on_64_kib_stack((*probe).to_string());
            assert_eq!(big, small, "probe {:?}", probe);
        }
    }

    /// Probes that together cover every construct the parse can descend into,
    /// plus the hostile-input shapes (unterminated groups, lone `$`, bare
    /// `\begin`, C0 separators, multi-byte text).
    const DIFFERENTIAL_PROBES: &[&str] = &[
        "",
        "   ",
        "x",
        "E = mc^2",
        "\\frac{a}{b}",
        "\\frac12",
        "\\frac{a}",
        "\\frac{\\frac{a}{b}}{\\frac{c}{d}}",
        "\\dfrac{1}{2}",
        "\\binom{n}{k}",
        "\\binom{\\binom{a}{b}}{c}",
        "\\sqrt{x}",
        "\\sqrt2",
        "\\sqrt[]{x}",
        "\\sqrt[3]{x}",
        "\\sqrt[\\sqrt[2]{3}]{x+y}",
        "\\sum_{i=1}^{n} i^2",
        "\\sum^{a}",
        "\\sum",
        "\\int_0^1 x^2 dx",
        "\\iint_D \\oint_C f",
        "\\bigcup_\\bigcap",
        "\\prod\\limits_{i}",
        "\\sum_{\\frac{a}{b}}^{\\sqrt{c}}",
        "\\left( \\frac{a}{b} \\right)",
        "\\left\\{ x \\right}",
        "\\left[ 0, 1 \\right]",
        "\\left. \\frac{d}{dx} \\right|_{x=0}",
        "\\left\\| x \\right\\|",
        "\\left(\\left(\\left(a\\right)\\right)\\right)",
        "\\left(x",
        "\\right)",
        "\\begin{matrix} 1 & 2 \\\\ 3 & 4 \\end{matrix}",
        "\\begin{pmatrix} 1 & 2 \\\\ 3 & 4 \\end{pmatrix}",
        "\\begin{bmatrix} a \\end{bmatrix}",
        "\\begin{vmatrix} 1 & 0 \\\\ 0 & 1 \\end{vmatrix}",
        "\\begin{cases} x & y \\\\ z & w \\end{cases}",
        "\\begin{cases}x\\end{cases}",
        "\\begin{aligned} a &= b \\\\ c &= d \\end{aligned}",
        "\\begin{aligned} a \\\\ \\end{aligned}",
        "\\begin{array}{cc} 1 & 2 \\end{array}",
        "\\begin{array}{} x \\end{array}",
        "\\begin{gather*} a \\end{gather*}",
        "\\begin{matrix}1\\end{matrix}\\begin{matrix}2\\end{matrix}",
        "\\begin",
        "\\begin{matrix}",
        "\\end{matrix}",
        "\\begin{matrix} \\frac{a}{b} & \\sqrt{c} \\\\ \\vec{d} & \\left(e\\right) \\end{matrix}",
        "\\begin{cases} \\frac{a}{b} & x>0 \\\\ \\sqrt{c} & x\\le 0 \\end{cases}",
        "\\begin{pmatrix} a \\end{pmatrix}_i^2",
        "\\left( \\begin{matrix} 1 \\\\ 2 \\end{matrix} \\right)",
        "\\vec{a} \\cdot \\hat{b}",
        "\\overline{AB}",
        "\\bar{\\overline{A}}",
        "\\overline",
        "\\boxed{E=mc^2}",
        "\\boxed{}",
        "\\text{hello} & \\mathrm{d}x \\mathbf{v} \\mathbb{R} \\mathcal{F} \\boldsymbol{\\alpha}",
        "\\text{a\\text{b}c}d",
        "\\text{}",
        "\\sin \\cos \\tan \\log_2 x \\lim_{x \\to 0} \\frac{1}{x}",
        "\\lim x",
        "\\max_{n} \\min_{m}",
        "\\lim_{\\lim_{\\lim_{x}}}",
        "\\alpha\\beta\\gamma\\delta",
        "\\,\\; \\quad\\qquad\\!",
        "\\% \\_ \\& \\# 100\\%",
        "50\\% \\& 'quote' <tag> \"double\"",
        "x_i^2",
        "{x_i}^2",
        "a_b^c^d",
        "^{2}x",
        "x^{a^{b}}",
        "^_",
        "a^",
        "_x",
        "x'_i",
        "{}^1_2H",
        "\\alpha_{1}^{n}",
        "$$x^2$$",
        "$x^2$",
        "$$",
        "$",
        "x$$y",
        "$$ x $$ and $ y $",
        "\\unknowncommand{x}",
        "\\LaTeX \\TeX \\TeXture",
        "\\cal{F}",
        "\\operatorname{sn} x",
        "\\overrightarrow{AB}",
        "a\\b",
        "a b",
        "   \\sum   ",
        "a\u{1c}b",
        "&&",
        "\\\\",
        "&",
        "Ω x",
        "\\frac{中}{文}",
        "\\sqrt{ä}",
        "\\sum_{i=1}^{\\infty} \\frac{1}{i^2} = \\frac{\\pi^2}{6}",
        "\\int_a^b f(x)\\,dx = F(b) - F(a)",
        "f(x) = \\begin{cases} 1 & x > 0 \\\\ 0 & x \\le 0 \\end{cases}",
        "A \\cap B \\cup C \\setminus D",
        "\\{ a \\} \\langle u | v \\rangle",
        "\\forall x \\exists y : x \\in S",
        "\\text{spin-up } \\uparrow \\text{ spin-down } \\downarrow",
        "\\begin{aligned} x &= \\begin{cases} \\frac{1}{2} & a \\end{cases} \\end{aligned}",
        "\\sqrt{\\sqrt{\\sqrt{\\sqrt{x}}}}",
        "\\left(\\left[\\left\\{x\\right\\}\\right]\\right)",
    ];

    /// The frame-stack parser must be byte-identical to the recursion it replaced.
    #[test]
    fn iterative_parser_matches_the_recursive_one_it_replaced() {
        let mut diffs = Vec::new();
        for probe in DIFFERENTIAL_PROBES {
            let trimmed = py_trim(probe);
            let iter = parse_latex_to_omml_inner(trimmed);
            let rec = parse_latex_to_omml_inner_recursive(trimmed);
            if iter != rec {
                diffs.push(format!(
                    "{:?}\n  recursive = {}\n  iterative = {}",
                    probe, rec, iter
                ));
            }
        }
        assert!(diffs.is_empty(), "{} probes disagree:\n{}", diffs.len(), diffs.join("\n--\n"));
    }

    /// Python 的 `latex_to_omml` 只有在输入**没有** `$$…$$` 外壳时才输出
    /// `<m:oMath>`：`latex2omml.py:426-428` 在 strip 后的输入以 `$$` 开头且结尾时
    /// 把 `is_block` 重新绑定成 `True`，所以 **inline 调用也返回 `<m:oMathPara …>`**
    /// （实测 `$$x^2$$` / `$$` / `$$$` 的 inline 与 block 字节相同，见
    /// scratch/rust_parity/latex_probe_shapes.txt 末段）。逐字符 golden 表里
    /// `("$$x^2$$", "<m:oMathPara …")` 记录的就是这个行为。
    fn python_promotes_to_block(src: &str) -> bool {
        let trimmed = py_trim(src);
        trimmed.starts_with("$$") && trimmed.ends_with("$$")
    }

    /// Every Python-derived golden, re-checked construct by construct against the
    /// recursive oracle at the inner-XML level (the goldens above already pin the
    /// full `latex_to_omml` bytes; this pins the function that was rewritten).
    #[test]
    fn every_golden_probe_survives_the_rewrite() {
        let math_open = format!("<m:oMath xmlns:m=\"{}\" xmlns:w=\"{}\">", _M_NS, _W_NS);
        let para_open = format!(
            "<m:oMathPara xmlns:m=\"{}\" xmlns:w=\"{}\"><m:oMath>",
            _M_NS, _W_NS
        );
        for probe in DIFFERENTIAL_PROBES {
            let inline = latex_to_omml(probe, false);
            let block = latex_to_omml(probe, true);
            // `is_block=True` 恒为 `<m:oMathPara …><m:oMath>inner</m:oMath></m:oMathPara>`。
            let inner = block
                .strip_prefix(para_open.as_str())
                .unwrap_or_else(|| {
                    panic!(
                        "{:?}: block did not open with <m:oMathPara …><m:oMath>: {}",
                        probe,
                        &block[..block.len().min(80)]
                    )
                });
            let inner = inner
                .strip_suffix("</m:oMath></m:oMathPara>")
                .unwrap_or_else(|| {
                    panic!("{:?}: block did not close </m:oMath></m:oMathPara>", probe)
                });
            // Python 的空兜底 `_r("")` 保证 inner 永不为空（latex2omml.py:433-434）。
            assert!(
                !inner.is_empty(),
                "{:?}: empty inner XML — the Python `<m:r><m:t></m:t></m:r>` fallback is gone",
                probe
            );
            if python_promotes_to_block(probe) {
                assert_eq!(
                    inline, block,
                    "{:?}: a `$$…$$` probe re-binds is_block, so inline must equal block",
                    probe
                );
            } else {
                assert!(inline.starts_with("<m:oMath "), "{:?}", probe);
                assert!(inline.ends_with("</m:oMath>"), "{:?}", probe);
                assert_eq!(
                    inline,
                    format!("{}{}</m:oMath>", math_open, inner),
                    "{:?}: inline must be the very same inner XML inside a namespace-carrying <m:oMath>",
                    probe
                );
            }
        }
    }

    /// Deterministic pseudo-random formulas across the whole grammar, re-checked
    /// against the recursive oracle — this is what would catch a descent that got
    /// re-ordered relative to a token consumption.
    #[test]
    fn iterative_parser_matches_recursive_on_generated_formulas() {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut bad = Vec::new();
        for _ in 0..3000 {
            let depth = (next() % 4) as u32 + 1;
            let src = generated_latex(&mut next, depth);
            let iter = parse_latex_to_omml_inner(&src);
            let rec = parse_latex_to_omml_inner_recursive(&src);
            if iter != rec {
                bad.push(format!(
                    "{}\n  recursive = {}\n  iterative = {}",
                    src, rec, iter
                ));
                if bad.len() >= 5 {
                    break;
                }
            }
        }
        assert!(bad.is_empty(), "{} generated formulas disagree:\n{}", bad.len(), bad.join("\n--\n"));
    }

    fn generated_atom<R: FnMut() -> u64>(rng: &mut R) -> String {
        match (rng() % 6) as usize {
            0 => "x".to_string(),
            1 => "1".to_string(),
            2 => "\\alpha".to_string(),
            3 => "\\sin".to_string(),
            4 => "\\le".to_string(),
            _ => "f(x)".to_string(),
        }
    }

    fn generated_latex<R: FnMut() -> u64>(rng: &mut R, depth: u32) -> String {
        if depth == 0 {
            return generated_atom(rng);
        }
        // The arm is picked *before* any child is generated, so the recursion
        // order — and therefore the byte stream the two parsers see — is fixed.
        let pick = (rng() % 19) as usize;
        match pick {
            0 => {
                let a = generated_latex(rng, depth - 1);
                let b = generated_latex(rng, depth - 1);
                format!("\\frac{{{}}}{{{}}}", a, b)
            }
            1 => {
                let a = generated_latex(rng, depth - 1);
                format!("\\dfrac{{{}}}{{{}}}", a, generated_atom(rng))
            }
            2 => {
                let a = generated_latex(rng, depth - 1);
                let b = generated_latex(rng, depth - 1);
                format!("\\binom{{{}}}{{{}}}", a, b)
            }
            3 => {
                let a = generated_latex(rng, depth - 1);
                format!("\\sqrt{{{}}}", a)
            }
            4 => {
                let a = generated_latex(rng, depth - 1);
                format!("\\sqrt[{}]{{{}}}", generated_atom(rng), a)
            }
            5 => {
                let a = generated_latex(rng, depth - 1);
                format!("{}^{{{}}}", generated_atom(rng), a)
            }
            6 => {
                let a = generated_latex(rng, depth - 1);
                format!("{}_{{{}}}", generated_atom(rng), a)
            }
            7 => {
                let a = generated_latex(rng, depth - 1);
                let b = generated_latex(rng, depth - 1);
                format!("{}_{{{}}}^{{{}}}", generated_atom(rng), a, b)
            }
            8 => {
                let a = generated_latex(rng, depth - 1);
                format!("\\left({} \\right)", a)
            }
            9 => {
                let a = generated_latex(rng, depth - 1);
                // built from raw literals: `\left\{` / `\right\}` would otherwise
                // need brace-escaping inside a format string
                let mut s = String::from(r"\left\{");
                s.push_str(&a);
                s.push_str(r" \right\}");
                s
            }
            10 => {
                let a = generated_latex(rng, depth - 1);
                let b = generated_latex(rng, depth - 1);
                let c = generated_latex(rng, depth - 1);
                let d = generated_latex(rng, depth - 1);
                format!("\\begin{{matrix}} {} & {} \\\\ {} & {} \\end{{matrix}}", a, b, c, d)
            }
            11 => {
                let a = generated_latex(rng, depth - 1);
                let b = generated_latex(rng, depth - 1);
                let c = generated_latex(rng, depth - 1);
                let d = generated_latex(rng, depth - 1);
                format!("\\begin{{cases}} {} & {} \\\\ {} & {} \\end{{cases}}", a, b, c, d)
            }
            12 => {
                let a = generated_latex(rng, depth - 1);
                let b = generated_latex(rng, depth - 1);
                let c = generated_latex(rng, depth - 1);
                let d = generated_latex(rng, depth - 1);
                format!("\\begin{{aligned}} {} &= {} \\\\ {} &= {} \\end{{aligned}}", a, b, c, d)
            }
            13 => {
                let a = generated_latex(rng, depth - 1);
                format!("\\vec{{{}}}", a)
            }
            14 => {
                let a = generated_latex(rng, depth - 1);
                format!("\\overline{{{}}}", a)
            }
            15 => {
                let a = generated_latex(rng, depth - 1);
                format!("\\boxed{{{}}}", a)
            }
            16 => {
                let a = generated_latex(rng, depth - 1);
                format!("\\lim_{{{}}} {}", a, generated_atom(rng))
            }
            17 => {
                let a = generated_latex(rng, depth - 1);
                let b = generated_latex(rng, depth - 1);
                format!("\\sum_{{{}}}^{{{}}} {}", a, b, generated_atom(rng))
            }
            _ => {
                let a = generated_latex(rng, depth - 1);
                format!("\\begin{{pmatrix}} {} & {} \\end{{pmatrix}}", a, generated_atom(rng))
            }
        }
    }
}
