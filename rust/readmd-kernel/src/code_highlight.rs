//! Small rule-based syntax highlighter for exported code blocks (DOCX, PDF).
//!
//! Not a full lexer: it recognises comments, strings, numbers, keywords and
//! capitalised type names for common languages, which is enough for readable
//! colour in print.  Every slice is taken on `char_indices` boundaries, so
//! multi-byte text never panics, and concatenating the tokens reproduces the
//! input exactly.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Plain,
    Keyword,
    String,
    Comment,
    Number,
    Type,
}

/// Print-friendly colours (light background).
pub fn color_of(k: Kind) -> Option<&'static str> {
    match k {
        Kind::Plain => None,
        Kind::Keyword => Some("0033B3"),
        Kind::String => Some("067D17"),
        Kind::Comment => Some("8C8C8C"),
        Kind::Number => Some("1750EB"),
        Kind::Type => Some("7A3E9D"),
    }
}

struct Lang {
    line_comments: &'static [&'static str],
    block_comment: Option<(&'static str, &'static str)>,
    keywords: &'static [&'static str],
    quotes: &'static [char],
    case_insensitive: bool,
}

const C_LIKE: &[&str] = &[
    "if", "else", "for", "while", "do", "switch", "case", "default", "break", "continue", "return", "goto", "struct",
    "union", "enum", "typedef", "const", "static", "extern", "void", "int", "char", "float", "double", "long", "short",
    "unsigned", "signed", "sizeof", "class", "public", "private", "protected", "new", "delete", "this", "true",
    "false", "null", "nullptr", "try", "catch", "throw", "namespace", "using", "template", "typename", "virtual",
    "override", "final", "import", "package", "interface", "extends", "implements", "var", "let", "function",
    "async", "await", "yield", "export", "from", "of", "in", "instanceof", "typeof", "undefined", "bool", "boolean",
    "string", "auto", "inline", "volatile", "go", "func", "defer", "chan", "map", "range", "select", "type",
    "fallthrough", "struct", "readonly", "abstract", "static", "super", "as", "is", "foreach", "lock", "out", "ref",
];
const RUST: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self",
    "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while", "Some", "None", "Ok",
    "Err",
];
const PYTHON: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
    "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal",
    "not", "or", "pass", "raise", "return", "try", "while", "with", "yield", "self", "print",
];
const SHELL: &[&str] = &[
    "if", "then", "else", "elif", "fi", "for", "in", "do", "done", "while", "until", "case", "esac", "function",
    "return", "export", "local", "echo", "cd", "exit", "set", "unset", "source", "sudo",
];
const SQL: &[&str] = &[
    "select", "from", "where", "and", "or", "not", "insert", "into", "values", "update", "set", "delete", "create",
    "table", "drop", "alter", "index", "join", "left", "right", "inner", "outer", "on", "group", "by", "order",
    "having", "limit", "offset", "as", "distinct", "null", "is", "in", "like", "primary", "key", "foreign",
    "references", "union", "all", "case", "when", "then", "else", "end", "asc", "desc",
];
const YAML: &[&str] = &["true", "false", "null", "yes", "no", "on", "off"];

fn lang_for(lang: &str) -> Option<Lang> {
    let l = lang.to_ascii_lowercase();
    Some(match l.as_str() {
        "rust" | "rs" => Lang { line_comments: &["//"], block_comment: Some(("/*", "*/")), keywords: RUST, quotes: &['"'], case_insensitive: false },
        "python" | "py" | "python3" => Lang { line_comments: &["#"], block_comment: None, keywords: PYTHON, quotes: &['"', '\''], case_insensitive: false },
        "bash" | "sh" | "shell" | "zsh" | "powershell" | "ps1" | "pwsh" => Lang { line_comments: &["#"], block_comment: None, keywords: SHELL, quotes: &['"', '\''], case_insensitive: false },
        "sql" | "mysql" | "postgres" | "sqlite" => Lang { line_comments: &["--"], block_comment: Some(("/*", "*/")), keywords: SQL, quotes: &['\''], case_insensitive: true },
        "yaml" | "yml" | "toml" | "ini" => Lang { line_comments: &["#"], block_comment: None, keywords: YAML, quotes: &['"', '\''], case_insensitive: false },
        "json" | "jsonc" => Lang { line_comments: &["//"], block_comment: None, keywords: &["true", "false", "null"], quotes: &['"'], case_insensitive: false },
        "html" | "xml" | "svg" | "vue" => Lang { line_comments: &[], block_comment: Some(("<!--", "-->")), keywords: &[], quotes: &['"', '\''], case_insensitive: false },
        "css" | "scss" | "less" => Lang { line_comments: &[], block_comment: Some(("/*", "*/")), keywords: &["important"], quotes: &['"', '\''], case_insensitive: false },
        "" | "text" | "txt" | "plain" | "plaintext" | "output" | "console" => return None,
        // C family, JS/TS, Java, Go, C#, Kotlin, Swift, PHP … and unknown languages.
        _ => Lang { line_comments: &["//"], block_comment: Some(("/*", "*/")), keywords: C_LIKE, quotes: &['"', '\'', '`'], case_insensitive: false },
    })
}

/// Split `code` into coloured tokens.  Unknown / plain languages yield one
/// `Plain` token.
pub fn tokenize<'a>(code: &'a str, lang: &str) -> Vec<(Kind, &'a str)> {
    let Some(l) = lang_for(lang) else {
        return if code.is_empty() { Vec::new() } else { vec![(Kind::Plain, code)] };
    };
    let mut out: Vec<(Kind, &'a str)> = Vec::new();
    let push = |out: &mut Vec<(Kind, &'a str)>, k: Kind, s: &'a str| {
        if !s.is_empty() {
            out.push((k, s));
        }
    };
    let bytes_len = code.len();
    let mut i = 0usize;
    let mut plain_start = 0usize;
    let flush_plain = |out: &mut Vec<(Kind, &'a str)>, from: usize, to: usize| {
        if to > from {
            push(out, Kind::Plain, &code[from..to]);
        }
    };
    while i < bytes_len {
        let rest = &code[i..];
        // comments
        if l.line_comments.iter().any(|c| rest.starts_with(*c)) {
            flush_plain(&mut out, plain_start, i);
            let end = rest.find('\n').map(|n| i + n).unwrap_or(bytes_len);
            push(&mut out, Kind::Comment, &code[i..end]);
            i = end;
            plain_start = i;
            continue;
        }
        if let Some((open, close)) = l.block_comment {
            if rest.starts_with(open) {
                flush_plain(&mut out, plain_start, i);
                let end = rest[open.len()..].find(close).map(|n| i + open.len() + n + close.len()).unwrap_or(bytes_len);
                push(&mut out, Kind::Comment, &code[i..end]);
                i = end;
                plain_start = i;
                continue;
            }
        }
        let c = rest.chars().next().unwrap_or(' ');
        // strings
        if l.quotes.contains(&c) {
            flush_plain(&mut out, plain_start, i);
            let mut j = i + c.len_utf8();
            let mut escaped = false;
            let mut closed = false;
            for (off, ch) in code[j..].char_indices() {
                if escaped {
                    escaped = false;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                if ch == '\n' && c != '`' {
                    j += off;
                    closed = true;
                    break;
                }
                if ch == c {
                    j += off + ch.len_utf8();
                    closed = true;
                    break;
                }
            }
            if !closed {
                j = bytes_len;
            }
            push(&mut out, Kind::String, &code[i..j]);
            i = j;
            plain_start = i;
            continue;
        }
        // numbers
        let prev_ident = code[..i].chars().last().map(|p| p.is_alphanumeric() || p == '_').unwrap_or(false);
        if c.is_ascii_digit() && !prev_ident {
            flush_plain(&mut out, plain_start, i);
            let mut j = i;
            for (off, ch) in rest.char_indices() {
                if ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' {
                    j = i + off + ch.len_utf8();
                } else {
                    break;
                }
            }
            push(&mut out, Kind::Number, &code[i..j]);
            i = j;
            plain_start = i;
            continue;
        }
        // identifiers
        if (c.is_alphabetic() || c == '_' || c == '$') && !prev_ident {
            let mut j = i;
            for (off, ch) in rest.char_indices() {
                if ch.is_alphanumeric() || ch == '_' || ch == '$' {
                    j = i + off + ch.len_utf8();
                } else {
                    break;
                }
            }
            let word = &code[i..j];
            let is_kw = if l.case_insensitive {
                l.keywords.iter().any(|k| k.eq_ignore_ascii_case(word))
            } else {
                l.keywords.contains(&word)
            };
            let is_type = !is_kw
                && word.len() > 1
                && word.chars().next().map(|f| f.is_ascii_uppercase()).unwrap_or(false)
                && word.chars().any(|ch| ch.is_ascii_lowercase());
            if is_kw || is_type {
                flush_plain(&mut out, plain_start, i);
                push(&mut out, if is_kw { Kind::Keyword } else { Kind::Type }, word);
                plain_start = j;
            }
            i = j;
            continue;
        }
        i += c.len_utf8();
    }
    flush_plain(&mut out, plain_start, bytes_len);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(code: &str, lang: &str) -> String {
        tokenize(code, lang).iter().map(|(_, s)| *s).collect()
    }

    #[test]
    fn tokens_reassemble_input() {
        for (code, lang) in [
            ("fn main() { let s = \"中文\"; // 注释\n}", "rust"),
            ("def f(x):\n    return 'a' + 1.5  # c", "python"),
            ("SELECT * FROM t WHERE a = 'x' -- c", "sql"),
            ("/* 未闭合", "js"),
            ("\"未闭合字符串", "c"),
            ("😀 emoji 1_000", "go"),
        ] {
            assert_eq!(joined(code, lang), code);
        }
    }

    #[test]
    fn classifies_common_tokens() {
        let t = tokenize("let x = 42; // hi", "rust");
        assert!(t.contains(&(Kind::Keyword, "let")));
        assert!(t.contains(&(Kind::Number, "42")));
        assert!(t.contains(&(Kind::Comment, "// hi")));
        let t = tokenize("print(\"s\")", "python");
        assert!(t.contains(&(Kind::String, "\"s\"")));
        assert!(t.contains(&(Kind::Keyword, "print")));
        assert_eq!(tokenize("plain", ""), vec![(Kind::Plain, "plain")]);
    }

    #[test]
    fn randomized_never_panics_and_reassembles() {
        let alphabet = ["中", "a", "1", "\"", "'", "`", "/", "*", "#", "-", "\n", " ", "\\", "😀", "<!--", "-->", "_"];
        let langs = ["rust", "python", "sql", "html", "js", "yaml", ""];
        let mut s: u64 = 42;
        for _ in 0..500 {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let n = (s >> 58) as usize;
            let mut code = String::new();
            for _ in 0..n {
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                code.push_str(alphabet[(s >> 33) as usize % alphabet.len()]);
            }
            let lang = langs[(s >> 20) as usize % langs.len()];
            assert_eq!(joined(&code, lang), code, "{code:?} / {lang}");
        }
    }
}
