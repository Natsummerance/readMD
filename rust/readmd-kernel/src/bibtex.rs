//! ReadMD BibTeX - 学术参考文献解析与交叉引用引擎。
//!
//! 支持:
//! 1. 自动扫描 Markdown 同目录或上级目录下的 .bib 文件
//! 2. 解析 @article, @book, @inproceedings, @techreport, @phdthesis, @misc 等条目
//! 3. 提取 Title, Author, Year/Date, Journal/Booktitle, Volume, Pages, DOI, URL
//! 4. 格式化学术引用短名 (如 "Vaswani et al., 2017") 与完整标准参考文献条目
//! 5. 零第三方依赖，纯标准库实现

use std::fs;
use std::path::{Path, PathBuf};

use crate::link_indexer::{py_abspath, py_dirname};

// ---------------------------------------------------------------------------
// CPython compatibility primitives (measured with CPython 3.11.15; see
// scratch/rust_parity/we9_probe.py).  `src/readmd_modules/bibtex.py` is the
// authority for every one of these: it uses bare `str.strip()`, bare
// `str.split()` and unicode-mode `re.\s`, all of which count U+001C..U+001F
// (file/group/record/unit separators) as whitespace on top of Unicode
// `White_Space`.  Rust's `char::is_whitespace`, `trim*`, `split_whitespace`
// and `regex`'s `\s` do not, so each of them is widened here.
// ---------------------------------------------------------------------------

/// Py `ch.isspace()`.  U+001A/U+001B stay non-space (measured), so widening
/// cannot swallow control characters that are not separators.
#[inline]
fn py_isspace(ch: char) -> bool {
    ch.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&ch)
}

/// Py `str.lstrip()`.
fn py_strip_start(s: &str) -> &str {
    s.trim_start_matches(py_isspace as fn(char) -> bool)
}

/// Py `str.rstrip()`.
#[allow(dead_code)]
fn py_strip_end(s: &str) -> &str {
    s.trim_end_matches(py_isspace as fn(char) -> bool)
}

/// Py `str.strip()`.
fn py_strip(s: &str) -> &str {
    s.trim_matches(py_isspace as fn(char) -> bool)
}

/// End of the leading non-whitespace token of `s` (a byte offset that is
/// always a char boundary, because it is produced from `char_indices`).
fn token_end(s: &str) -> usize {
    for (i, c) in s.char_indices() {
        if py_isspace(c) {
            return i;
        }
    }
    s.len()
}

/// Py `str.split()` with no separator: whitespace runs are the delimiters, so
/// U+001C separates two words just as `char::is_whitespace()`-class space does
/// (`'a\x1cb c'.split() == ['a', 'b', 'c']`, measured).
fn py_split(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        rest = py_strip_start(rest);
        if rest.is_empty() {
            break;
        }
        let end = token_end(rest);
        out.push(&rest[..end]);
        rest = &rest[end..];
    }
    out
}

/// Step exactly one code point from a known char boundary, saturating at the
/// end of the string — CPython's `pos = end_idx + 1` on a `str`.
fn py_step_one(s: &str, at: usize) -> usize {
    s[at..]
        .char_indices()
        .nth(1)
        .map(|(off, _)| at + off)
        .unwrap_or(s.len())
}

/// How many code points precede the byte offset `at`.  Used to re-base a
/// regex byte offset into the char-vector index space the field scanner works
/// in, so the two units are never mixed.
fn chars_before(s: &str, at: usize) -> usize {
    s[..at].chars().count()
}

/// BibTeX 条目字段数据
#[derive(Debug, Clone, Default)]
pub struct BibEntry {
    /// 条目类型 (article/book/inproceedings 等)
    pub entry_type: String,
    /// 引用键 (cite key)
    pub cite_key: String,
    /// 作者列表
    pub author: String,
    /// 年份
    pub year: String,
    /// 日期
    pub date: String,
    /// 标题
    pub title: String,
    /// 期刊/会议名称
    pub journal: String,
    /// 书籍名称
    pub booktitle: String,
    /// 出版商
    pub publisher: String,
    /// 卷号
    pub volume: String,
    /// 页码
    pub pages: String,
    /// DOI
    pub doi: String,
    /// URL
    pub url: String,
    /// 短引用文本 [Author et al., YEAR]
    pub short_cite: String,
    /// 完整参考文献格式化文本
    pub full_reference: String,
}

impl BibEntry {
    /// 从字段映射创建条目
    pub fn from_fields(fields: &std::collections::HashMap<String, String>) -> Self {
        let entry_type = fields.get("entry_type").cloned().unwrap_or_default();
        let cite_key = fields.get("cite_key").cloned().unwrap_or_default();
        let author = fields.get("author").cloned().unwrap_or_default();
        let year = fields.get("year").cloned().unwrap_or_default();
        let date = fields.get("date").cloned().unwrap_or_default();
        let title = fields.get("title").cloned().unwrap_or_default();
        let journal = fields.get("journal").cloned().unwrap_or_default();
        let booktitle = fields.get("booktitle").cloned().unwrap_or_default();
        let publisher = fields.get("publisher").cloned().unwrap_or_default();
        let volume = fields.get("volume").cloned().unwrap_or_default();
        let pages = fields.get("pages").cloned().unwrap_or_default();
        let doi = fields.get("doi").cloned().unwrap_or_default();
        let url = fields.get("url").cloned().unwrap_or_default();

        // `bibtex.py:137`: the short-cite year is `fields['year'] or fields['date']`
        // — an empty/absent year falls back to the date.  The stored `year` field
        // and `format_full_reference` (`bibtex.py:172`, which reads only
        // `fields['year']`) keep the raw value; only the citation label resolves
        // the fallback.
        let cite_year = if year.is_empty() { date.as_str() } else { year.as_str() };
        let short_cite = format_short_cite(&author, cite_year, &cite_key);
        let full_reference = format_full_reference(
            &author,
            &year,
            &title,
            &journal,
            &booktitle,
            &publisher,
            &volume,
            &pages,
            &doi,
        );

        BibEntry {
            entry_type,
            cite_key,
            author,
            year,
            date,
            title,
            journal,
            booktitle,
            publisher,
            volume,
            pages,
            doi,
            url,
            short_cite,
            full_reference,
        }
    }
}

/// 解析单个 .bib 文件并返回字典 {cite_key: entry}
pub fn parse_bibtex_file(file_path: &Path) -> std::collections::HashMap<String, BibEntry> {
    if !file_path.is_file() {
        return std::collections::HashMap::new();
    }

    // Py 24: `open(file_path, 'r', encoding='utf-8', errors='replace')`.  A byte
    // that is not valid UTF-8 becomes a single U+FFFD and parsing *continues*;
    // it never aborts the file.  `read_to_string` would hard-fail on the same
    // input and drop every entry, so the read is done as bytes and decoded
    // lossily (`String::from_utf8_lossy` emits one U+FFFD per maximal invalid
    // subpart, the same rule CPython's `replace` handler applies).
    let content = match fs::read(file_path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(e) => {
            eprintln!("Warning: Read bib file failed: {} ({})", file_path.display(), e);
            return std::collections::HashMap::new();
        }
    };

    parse_bibtex_content(&content)
}

/// 解析 BibTeX 内容字符串
pub fn parse_bibtex_content(content: &str) -> std::collections::HashMap<String, BibEntry> {
    let mut entries = std::collections::HashMap::new();
    
    // 正则匹配 @type{key, fields...}
    // `bibtex.py:32` compiles this without `re.ASCII`, so its `\s` is the
    // Unicode class *plus* U+001C..U+001F (measured: `re.match(r'\s','\x1c')`
    // matches); the Rust crate's `\s` is Unicode `White_Space` only, hence the
    // explicit `[\s\x1c-\x1f]` here and in the negated key class.
    let pattern = regex::Regex::new(
        r"@([a-zA-Z]+)[\s\x1c-\x1f]*\{[\s\x1c-\x1f]*([^\s\x1c-\x1f,]+)[\s\x1c-\x1f]*,",
    )
    .unwrap();
    
    let mut pos = 0;
    
    while let Some(m) = pattern.find_at(&content, pos) {
        // 使用 pattern.captures_at 来获取捕获组
        let caps = pattern.captures_at(&content, m.start()).unwrap();
        let entry_type = caps.get(1).map_or(String::new(), |m| m.as_str().to_lowercase());
        // Py 40 `m.group(2).strip()`
        let cite_key = caps.get(2).map_or(String::new(), |m| py_strip(m.as_str()).to_string());
        
        // 寻找匹配的闭合大括号.  `bibtex.py:43-56` walks *code points*, but every
        // index here has to address `&str`, so the walk stays in byte space over
        // a slice that is guaranteed to begin on a char boundary (`m.end()` is a
        // match end).  Feeding a byte offset into a `Vec<char>` — as the first
        // port did — mis-sliced and could panic mid-character.
        let start_idx = m.end();
        let mut brace_count = 1;
        let mut end_idx = start_idx;
        
        for (off, char) in content[start_idx..].char_indices() {
            match char {
                '{' => brace_count += 1,
                '}' => {
                    brace_count -= 1;
                    if brace_count == 0 {
                        end_idx = start_idx + off;
                        break;
                    }
                }
                _ => {}
            }
        }
        
        let body = &content[start_idx..end_idx];
        // Py 57 `pos = end_idx + 1`.  A found `}` is one byte, so the byte step is
        // exact; with no closing brace `end_idx` still equals `start_idx` and the
        // step must be one *code point*, otherwise `find_at` is handed a byte
        // inside a multi-byte character and panics.
        pos = if end_idx > start_idx {
            end_idx + 1
        } else {
            py_step_one(content, start_idx)
        };
        
        if entry_type == "comment" {
            continue;
        }
        
        // 解析字段 key = {value} 或 key = "value" 或 key = 123
        let mut fields = parse_bib_fields(body);
        fields.insert("entry_type".to_string(), entry_type.clone());
        fields.insert("cite_key".to_string(), cite_key.clone());
        
        let entry = BibEntry::from_fields(&fields);
        entries.insert(cite_key, entry);
    }
    
    entries
}

/// 解析 BibTeX 条目内部的 key = value 字段
fn parse_bib_fields(body: &str) -> std::collections::HashMap<String, String> {
    let mut fields = std::collections::HashMap::new();
    // Py 79: unicode-mode `\s`, i.e. the widened class.
    let field_pattern =
        regex::Regex::new(r"([a-zA-Z0-9_\-]+)[\s\x1c-\x1f]*=[\s\x1c-\x1f]*").unwrap();
    
    let mut pos = 0;
    let chars: Vec<char> = body.chars().collect();
    
    while pos < chars.len() {
        let remaining = chars[pos..].iter().collect::<String>();
        if let Some(m) = field_pattern.find(&remaining) {
            let caps = field_pattern.captures_at(&remaining, m.start()).unwrap();
            // Py 86 `m.group(1).lower().strip()`
            let key = caps.get(1).map_or(String::new(), |m| {
                py_strip(&m.as_str().to_lowercase()).to_string()
            });
            // `m.end()` is a BYTE offset into `remaining`; `pos` counts CODE
            // POINTS.  Re-base before adding or every later `chars[..]` index is
            // wrong once the body holds a single non-ASCII character.
            let val_start = pos + chars_before(&remaining, m.end());
            
            let (val, consumed) = if val_start < chars.len() {
                let first_char = chars[val_start];
                match first_char {
                    '{' => extract_braced_value(&chars, val_start),
                    '"' => extract_quoted_value(&chars, val_start),
                    _ => extract_unquoted_value(&chars, val_start),
                }
            } else {
                (String::new(), 0)
            };
            
            // Py 126 `re.sub(r'[\r\n\t]+', ' ', val).strip()` — runs collapse to
            // ONE space, and the strip is CPython's (it also eats U+001C..U+001F).
            let clean_val = py_strip(&py_crlf_runs(&val)).to_string();
            let clean_val = clean_val.replace('{', "").replace('}', "");
            
            fields.insert(key, clean_val);
            pos = val_start + consumed.max(1);
        } else {
            break;
        }
    }
    
    fields
}

/// Py `re.sub(r'[\r\n\t]+', ' ', s)`: each maximal run of CR/LF/TAB becomes a
/// single space.  Chained `replace` calls (the first port) turned `\r\n\t` into
/// three spaces.  Characters outside the run class — including the C0
/// separators — are passed through untouched, exactly like Python.
fn py_crlf_runs(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_run = false;
    for c in s.chars() {
        if c == '\r' || c == '\n' || c == '\t' {
            if !in_run {
                out.push(' ');
                in_run = true;
            }
        } else {
            in_run = false;
            out.push(c);
        }
    }
    out
}

/// 提取花括号内的值，返回 (内容, 消耗的字符数)
///
/// Literal mirror of `bibtex.py:94-106`.  `start` is the index of the opening
/// `{`.  `b_count` begins at 1; the scan stops on the `}` that returns it to 0
/// and returns the raw interior slice (nested braces survive verbatim — the
/// caller strips them later, exactly as `bibtex.py:127` does).  When no closer
/// exists, Python leaves `val == ''` and does NOT advance past the value, so we
/// return an empty value and 0 consumed (the caller then advances by one, which
/// is safe because the next body character is the non-matching `{`).
fn extract_braced_value(chars: &[char], start: usize) -> (String, usize) {
    let mut b_count = 1usize;
    let mut i = start + 1;
    while i < chars.len() {
        match chars[i] {
            '{' => b_count += 1,
            '}' => {
                b_count -= 1;
                if b_count == 0 {
                    let val: String = chars[start + 1..i].iter().collect();
                    return (val, i - start + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }
    (String::new(), 0)
}

/// 提取引号内的值，返回 (内容, 消耗的字符数)
///
/// Literal mirror of `bibtex.py:107-114`.  `start` is the index of the opening
/// `"`.  The terminator is the first `"` whose *preceding* character is not a
/// backslash (`body[i] == '"' and body[i - 1] != '\\'`).  The value is returned
/// as the untouched interior slice, so backslashes are preserved and `\"`
/// sequences stay verbatim.  The first port used an `escaped` state machine that
/// dropped every backslash and mis-detected `\` and `\\"`, which is exactly the
/// defect fixed here.  With no terminator Python leaves `val == ''` and does not
/// advance, so we return `(String::new(), 0)`.
fn extract_quoted_value(chars: &[char], start: usize) -> (String, usize) {
    let mut i = start + 1;
    while i < chars.len() {
        if chars[i] == '"' && chars[i - 1] != '\\' {
            let val: String = chars[start + 1..i].iter().collect();
            return (val, i - start + 1);
        }
        i += 1;
    }
    (String::new(), 0)
}

/// 提取无括号值 (直到逗号或换行)，返回 (内容, 消耗的字符数)
fn extract_unquoted_value(chars: &[char], start: usize) -> (String, usize) {
    let mut result = String::new();
    let mut consumed = 0;
    
    for &char in chars.iter().skip(start) {
        match char {
            ',' | '}' | '\n' | '\r' => break,
            _ => {
                consumed += 1;
                result.push(char);
            }
        }
    }
    
    // Py 119/122 `body[...].strip()`
    (py_strip(&result).to_string(), consumed)
}

/// 生成短学术引用标签，例如 [Vaswani et al., 2017]
fn format_short_cite(author: &str, year: &str, cite_key: &str) -> String {
    // Py 136 `author = fields.get('author', '').strip()` — the strip happens
    // *before* falsiness is tested, so a separator-only author is `not author`.
    let author = py_strip(author);
    if author.is_empty() {
        return format!("[{}]", cite_key);
    }
    
    // 分割多个作者 (BibTeX 中以 and 分隔)
    // Py 145 splits on the five-character literal `' and '`, not on the word
    // `and`: `'Smith, Jand Doe, R'.split(' and ')` stays one author (measured).
    let authors: Vec<&str> = author
        .split(" and ")
        .map(py_strip)
        .filter(|a| !a.is_empty())
        .collect();
    
    if authors.is_empty() {
        return format!("[{}]", cite_key);
    }
    
    let first_author = authors[0];
    // 处理 "Lastname, Firstname" 格式
    // `,` is a one-byte ASCII character, so the byte offset `find` reports is a
    // char boundary and `[..pos]` is exactly Py 152's `split(',')[0]`.
    let first_last = if let Some(pos) = first_author.find(',') {
        py_strip(&first_author[..pos]).to_string()
    } else {
        // Py 154-155 `parts = first_author.split(); parts[-1] if parts else ...`
        // — bare `split()` separates on U+001C..U+001F as well.
        let parts = py_split(first_author);
        parts.last().copied().unwrap_or(first_author).to_string()
    };
    
    let year_part = if !year.is_empty() {
        format!(", {}", year)
    } else {
        String::new()
    };
    
    let cite_str = match authors.len() {
        1 => format!("{}{}", first_last, year_part),
        2 => {
            let second_author = authors[1];
            let second_last = if let Some(pos) = second_author.find(',') {
                py_strip(&second_author[..pos]).to_string()
            } else {
                // Py 161 indexes `[-1]` unguarded; an author that survived
                // `if a.strip()` always has at least one `split()` token, so the
                // `unwrap_or` below is unreachable in the same way.
                let parts = py_split(second_author);
                parts.last().copied().unwrap_or(second_author).to_string()
            };
            format!("{} & {}{}", first_last, second_last, year_part)
        }
        _ => format!("{} et al.{}", first_last, year_part),
    };
    
    format!("[{}]", cite_str)
}

/// 格式化标准参考文献条目文本 (APA/IEEE 风格)
fn format_full_reference(
    author: &str,
    year: &str,
    title: &str,
    journal: &str,
    booktitle: &str,
    publisher: &str,
    volume: &str,
    pages: &str,
    doi: &str,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    // `bibtex.py:181` appends the (already " and "->", " joined) author WITH a
    // trailing period: `parts.append(f"{author}.")`.  The port dropped that
    // period, so every multi-field reference was off by one character; the
    // golden `article_basic` / `latin1_e` cases pin the period back on.
    if !author.is_empty() {
        parts.push(format!("{}.", author.replace(" and ", ", ")));
    }
    if !year.is_empty() {
        parts.push(format!("({}).", year));
    }
    if !title.is_empty() {
        parts.push(format!("{}.", title));
    }
    
    let venue = if !journal.is_empty() {
        Some(journal)
    } else if !booktitle.is_empty() {
        Some(booktitle)
    } else if !publisher.is_empty() {
        Some(publisher)
    } else {
        None
    };
    if let Some(venue_str) = venue {
        let mut j_part = format!("_{}_", venue_str);
        if !volume.is_empty() {
            j_part.push_str(&format!(", {}", volume));
        }
        if !pages.is_empty() {
            j_part.push_str(&format!(", {}", pages));
        }
        parts.push(j_part + ".");
    }
    
    if !doi.is_empty() {
        parts.push(format!("https://doi.org/{}", doi));
    }
    
    parts.join(" ")
}

/// 自动查找并载入 Markdown 文件同级或上级目录的 .bib 文件
///
/// Literal mirror of `bibtex.py:199-227`.  The first port added a "recurse up
/// to 3 levels" scan and always took `.parent()` of the argument (so a directory
/// argument scanned its parent); the authority performs no recursion and treats
/// a non-file argument as the directory itself.  `base_dir` is derived through
/// `os.path.abspath`/`os.path.dirname` so a bare relative filename still yields
/// the real containing directory (`.parent()` of `"foo.md"` is the empty path,
/// which is not a directory).
pub fn find_and_load_bib_for_file(markdown_file_path: &Path) -> std::collections::HashMap<String, BibEntry> {
    // Py 201: `if not markdown_file_path: return {}`
    let path_str = markdown_file_path.to_string_lossy();
    if path_str.is_empty() {
        return std::collections::HashMap::new();
    }

    // Py 204: file -> dirname(abspath(file)), otherwise abspath(the path itself)
    let base_dir = if markdown_file_path.is_file() {
        py_dirname(&py_abspath(&path_str))
    } else {
        py_abspath(&path_str)
    };

    // Py 205: `if not os.path.isdir(base_dir): return {}`
    if !Path::new(&base_dir).is_dir() {
        return std::collections::HashMap::new();
    }

    let mut bib_files: Vec<PathBuf> = Vec::new();

    // Py 210-212: every *.bib entry directly inside base_dir.  Python does not
    // pre-check isfile here; a directory named `x.bib` still fails inside
    // `parse_bibtex_file`'s own `os.path.isfile` guard, so we match that too.
    if let Ok(entries) = fs::read_dir(&base_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.to_lowercase().ends_with(".bib") {
                bib_files.push(Path::new(&base_dir).join(&name));
            }
        }
    }

    // Py 215-220: only when base_dir had none, scan exactly ONE parent.
    if bib_files.is_empty() {
        let parent_dir = py_dirname(&base_dir);
        if Path::new(&parent_dir).is_dir() && parent_dir != base_dir {
            if let Ok(entries) = fs::read_dir(&parent_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.to_lowercase().ends_with(".bib") {
                        bib_files.push(Path::new(&parent_dir).join(&name));
                    }
                }
            }
        }
    }

    let mut all_citations = std::collections::HashMap::new();
    for bf in bib_files {
        let parsed = parse_bibtex_file(&bf);
        all_citations.extend(parsed);
    }

    all_citations
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_article() {
        let content = r#"
@article{vaswani2017attention,
    author = "Vaswani, Ashish and Shazeer, Noam and Parmar, Niki and others",
    title = "Attention Is All You Need",
    year = "2017",
    journal = "Advances in Neural Information Processing Systems",
    volume = "30",
    pages = "6000-6010"
}"#;
        
        let entries = parse_bibtex_content(content);
        assert!(entries.contains_key("vaswani2017attention"));
        
        let entry = entries.get("vaswani2017attention").unwrap();
        assert_eq!(entry.entry_type, "article");
        assert_eq!(entry.year, "2017");
        assert!(entry.short_cite.contains("Vaswani"));
        assert!(entry.short_cite.contains("et al."));
    }
    
    #[test]
    fn test_parse_book() {
        let content = r#"
@book{knuth1984,
    author = "Knuth, Donald E.",
    title = "The Art of Computer Programming, Volume 1",
    year = "1984",
    publisher = "Addison-Wesley"
}"#;
        
        let entries = parse_bibtex_content(content);
        assert!(entries.contains_key("knuth1984"));
        
        let entry = entries.get("knuth1984").unwrap();
        assert_eq!(entry.author, "Knuth, Donald E.");
        assert!(entry.short_cite.contains("Knuth, 1984"));
    }
    
    #[test]
    fn test_find_bib_files() {
        let temp_dir = std::env::temp_dir().join(format!("readmd-bibtex-test-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        // 创建测试 .bib 文件
        let bib_content = r#"
@article{test2024,
    author = "Test, Author",
    title = "Test Paper",
    year = "2024"
}"#;
        let bib_path = temp_dir.join("references.bib");
        std::fs::write(&bib_path, bib_content).unwrap();
        
        // 创建测试 markdown 文件
        let md_path = temp_dir.join("test.md");
        std::fs::write(&md_path, "# Test").unwrap();
        
        let citations = find_and_load_bib_for_file(&md_path);
        assert!(citations.contains_key("test2024"));
        
        // 清理
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_format_citations() {
        let mut fields = std::collections::HashMap::new();
        fields.insert("author".to_string(), "Smith, John and Doe, Jane".to_string());
        fields.insert("year".to_string(), "2023".to_string());
        fields.insert("cite_key".to_string(), "smith2023".to_string());
        
        let entry = BibEntry::from_fields(&fields);
        assert!(entry.short_cite.contains("Smith & Doe, 2023"));
    }
}

// ===========================================================================
// W-E9 whitespace-parity tests.  Expected values are CPython's own answers,
// measured with the mirrors in scratch/rust_parity/we9_expect.py (authority:
// src/readmd_modules/bibtex.py).  Do not "adjust" them to match the port.
// ===========================================================================
#[cfg(test)]
mod we9_tests {
    use super::*;
    use std::panic::catch_unwind;

    /// The ten code points this wave has to cover, plus the empty string.
    const WS: [&str; 10] = [
        "\u{1c}", "\u{1d}", "\u{1e}", "\u{1f}", "\u{a0}", "\u{3000}", "\u{2028}",
        "\t", "\r", "",
    ];

    fn fields_of(src: &str) -> Vec<(String, BibEntry)> {
        parse_bibtex_content(src).into_iter().collect()
    }

    #[test]
    fn we9_helper_set_is_cpython_exact() {
        // measured we9_probe #1/#2/#3: 1c..1f and every White_Space code point are
        // space; 1a/1b are NOT (so widening cannot swallow the placeholder sentinels)
        for c in ['\u{1c}', '\u{1d}', '\u{1e}', '\u{1f}', '\u{a0}', '\u{3000}',
                  '\u{2028}', '\u{2029}', '\u{85}', '\u{b}', '\u{c}', '\t', '\r', '\n', ' '] {
            assert!(py_isspace(c), "U+{:04X} must be space", c as u32);
        }
        for c in ['\u{1a}', '\u{1b}', 'x', '\u{0}'] {
            assert!(!py_isspace(c), "U+{:04X} must NOT be space", c as u32);
        }
        assert_eq!(py_strip("\u{1c} \u{1d}foo \u{1e}\u{a0}\u{3000}\u{1f}"), "foo");
        assert_eq!(py_strip_start("\u{1c}\u{1d}foo\u{1e}"), "foo\u{1e}");
        assert_eq!(py_strip_end("\u{1c}foo\u{1e}\u{1f}"), "\u{1c}foo");
        assert_eq!(py_strip(""), "");
        // measured #8: bare `split()` separates on U+001C
        assert_eq!(py_split("a\u{1c}b c\td"), vec!["a", "b", "c", "d"]);
        assert!(py_split(" \u{1c} ").is_empty());
        assert!(py_split("").is_empty());
        assert_eq!(py_split("Single\u{1f}").last().copied().unwrap_or(""), "Single");
        // measured #16 vs #17: a CR/LF/TAB RUN collapses to one space
        assert_eq!(py_crlf_runs("a\r\n\td\u{1c} e"), "a d\u{1c} e");
        assert_eq!(py_strip(&py_crlf_runs("a\r\n\td\u{1c} e")), "a d\u{1c} e");
        assert_eq!(py_crlf_runs(""), "");
        assert_eq!(py_step_one("a\u{a0}b", 1), 3);
        assert_eq!(py_step_one("ab", 2), 2);
        assert_eq!(chars_before("a\u{3000}b", 4), 2);
    }

    #[test]
    fn we9_cite_key_eats_separators_like_python() {
        // Py 32 `\s*` around the key: `@article{ \x1ckey\x1d ,` -> group(2) == 'key'
        let e = parse_bibtex_content("@article{ \u{1c}key\u{1d} ,\n title={X}\n}");
        assert!(e.contains_key("key"), "keys={:?}", e.keys().collect::<Vec<_>>());
        assert_eq!(e.get("key").unwrap().cite_key, "key");
        // a trailing separator on the key is not part of the key either
        let e2 = parse_bibtex_content("@article{key\u{1c},\n title={X}\n}");
        assert!(e2.contains_key("key"));
    }

    #[test]
    fn we9_field_pattern_eats_separators_like_python() {
        // Py 79: `author\x1c= {v}` and `author = \x1c{v}` both key the value `author`
        for src in [
            "@article{k,\n author\u{1c}= {Vaswani, Ashish}\n}",
            "@article{k,\n author =\u{1c}{Vaswani, Ashish}\n}",
            "@article{k,\n author\u{1c}=\u{1d}{Vaswani, Ashish}\n}",
        ] {
            let e = parse_bibtex_content(src);
            assert_eq!(e.get("k").map(|x| x.author.as_str()), Some("Vaswani, Ashish"), "{:?}", src);
        }
    }

    #[test]
    fn we9_separators_collapse_author_to_bare_key() {
        // measured: author.strip() == '' -> `[cite_key]`, no year part at all
        let e = parse_bibtex_content("@article{k,\n author = \"\u{1c}\u{1d}\",\n year = {2020}\n}");
        assert_eq!(e.get("k").unwrap().author, "");
        assert_eq!(e.get("k").unwrap().short_cite, "[k]");
        // from_fields sees the raw author, so the entry-level strip matters too
        let mut f = std::collections::HashMap::new();
        f.insert("author".to_string(), " \u{a0} ".to_string());
        f.insert("cite_key".to_string(), "kk".to_string());
        assert_eq!(BibEntry::from_fields(&f).short_cite, "[kk]");
        let mut f2 = std::collections::HashMap::new();
        f2.insert("author".to_string(), "\u{1c}\u{1d}\u{1e}\u{1f}".to_string());
        f2.insert("year".to_string(), "2000".to_string());
        f2.insert("cite_key".to_string(), "kk".to_string());
        assert_eq!(BibEntry::from_fields(&f2).short_cite, "[kk]");
    }

    #[test]
    fn we9_author_splits_on_literal_and_not_on_the_word_and() {
        // measured: 'Smith, Jand Doe, R'.split(' and ') -> one author -> 'Smith'
        let mut f = std::collections::HashMap::new();
        f.insert("author".to_string(), "Smith, Jand Doe, R".to_string());
        f.insert("cite_key".to_string(), "kk".to_string());
        assert_eq!(BibEntry::from_fields(&f).short_cite, "[Smith]");
        // 'Sanderson' must stay one surname
        let mut f2 = std::collections::HashMap::new();
        f2.insert("author".to_string(), "Sanderson".to_string());
        f2.insert("cite_key".to_string(), "kk".to_string());
        assert_eq!(BibEntry::from_fields(&f2).short_cite, "[Sanderson]");
        // the genuine delimiter still splits
        let mut f3 = std::collections::HashMap::new();
        f3.insert("author".to_string(), "One Two and Three Four".to_string());
        f3.insert("year".to_string(), "1999".to_string());
        f3.insert("cite_key".to_string(), "kk".to_string());
        assert_eq!(BibEntry::from_fields(&f3).short_cite, "[Two & Four, 1999]");
        // measured: an author that is only a separator between ` and ` runs is dropped
        let mut f4 = std::collections::HashMap::new();
        f4.insert("author".to_string(), "Smith, J and \u{1c} and Doe, R".to_string());
        f4.insert("cite_key".to_string(), "kk".to_string());
        assert_eq!(BibEntry::from_fields(&f4).short_cite, "[Smith & Doe]");
    }

    #[test]
    fn we9_surname_fallback_uses_cpython_split() {
        // measured: 'A\x1cB' -> split() -> ['A','B'] -> last 'B'
        let e = parse_bibtex_content("@article{k,\n author = {A\u{1c}B},\n year = {2017}\n}");
        assert_eq!(e.get("k").unwrap().author, "A\u{1c}B");
        assert_eq!(e.get("k").unwrap().short_cite, "[B, 2017]");
        // measured: 'A  B\x1cC D' -> last token 'D'
        let e2 = parse_bibtex_content("@article{k,\n author = \"A  B\u{1c}C D\",\n year = {}\n}");
        assert_eq!(e2.get("k").unwrap().short_cite, "[D]");
        // measured: two authors, both separator-separated
        let e3 = parse_bibtex_content("@article{k, author = {a\u{1c}b and c\u{1d}d}, year={5}}");
        assert_eq!(e3.get("k").unwrap().short_cite, "[b & d, 5]");
    }

    #[test]
    fn we9_values_strip_separators_at_the_edges_only() {
        // measured: `{U\x1c}` -> 'U' (trailing separator stripped by .strip())
        let e = parse_bibtex_content("@article{k, title = {U\u{1c}}, author={Smith, J and Doe, R and Foo}}");
        assert_eq!(e.get("k").unwrap().title, "U");
        assert_eq!(e.get("k").unwrap().short_cite, "[Smith et al.]");
        // measured: a CR/LF/TAB run collapses to ONE space, interior \x1c survives
        let e2 = parse_bibtex_content("@misc{m,\n title = \"a\r\n\tb\u{1c} c\"\n}");
        assert_eq!(e2.get("m").unwrap().title, "a b\u{1c} c");
        // unquoted values: Py 119/122 .strip()
        let e3 = parse_bibtex_content("@article{k,\n year = \u{1e}2017\u{1f},\n title = {T}\n}");
        assert_eq!(e3.get("k").unwrap().year, "2017");
    }

    #[test]
    fn we9_non_ascii_body_does_not_shift_the_scan() {
        // bibtex.py slices by CODE POINT; the port must not feed byte offsets to a
        // char index.  Un\u{ef}code / M\u{fc}ller put multi-byte text on both sides.
        let src = "@article{k,\ntitle={\u{dc}n\u{ef}code},\nauthor={M\u{fc}ller, A and \u{c9}, X}\nyear={2017}\n}";
        let e = fields_of(src);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].1.title, "\u{dc}n\u{ef}code");
        assert_eq!(e[0].1.author, "M\u{fc}ller, A and \u{c9}, X");
        assert_eq!(e[0].1.short_cite, "[M\u{fc}ller & \u{c9}, 2017]");
        // second entry after a non-ASCII first entry still found
        let two = format!("{}\n@book{{{},{}}}", src, "b2", " author = {Zhang, San}, year = {2020}");
        let e2 = parse_bibtex_content(&two);
        assert!(e2.contains_key("b2"), "keys={:?}", e2.keys().collect::<Vec<_>>());
        assert_eq!(e2.get("b2").unwrap().short_cite, "[Zhang, 2020]");
    }

    #[test]
    fn we9_matrix_never_panics() {
        // 10 whitespace code points (incl. the empty string) x 5 shapes, each run
        // through the public entry point.  Pre-fix this could panic on
        // `byte index N is not a char boundary`.
        let mut runs = 0usize;
        for w in WS.iter() {
            for tmpl in [
                format!("@article{{k,\n author = {{A{0}B}},\n year = {{2017}}\n}}", w),
                format!("@article{{ {0}key{0} ,\n title = {{{0}}}\n}}", w),
                format!("@misc{0}m{0}, note = \"a{0}b\"\n}}", w),
                format!("@article{{k, title = {{U{0}", w),
                format!("@comment{{{0} junk\n}}\n@misc{{m,\n year = {0}19{0}\n}}", w),
            ] {
                runs += 1;
                let src = tmpl.clone();
                let r = catch_unwind(move || parse_bibtex_content(&src));
                assert!(r.is_ok(), "panic on {:?}", tmpl);
            }
        }
        assert_eq!(runs, 50);
        // and the empty document is simply empty
        assert!(parse_bibtex_content("").is_empty());
    }

    #[test]
    fn we9_entry_type_and_comment_still_work() {
        // measured: `@comment{...}` is skipped, `@ARTICLE` lower-cases
        let e = parse_bibtex_content("@comment{\u{1c} junk\n}\n@ARTICLE{m,\n year = {1}\n}");
        assert_eq!(e.len(), 1);
        assert_eq!(e.get("m").unwrap().entry_type, "article");
    }
}

// ===========================================================================
// Golden differential tests.  Both JSON files are produced by running the
// *Python authority* `src/readmd_modules/bibtex.py` offline through
// scratch/rust_parity/bibtex_epub_s15/gen_goldens.py.  The expected values are
// CPython's own answers, so a green run here proves parity with the authority,
// not with the port.
// ===========================================================================
#[cfg(test)]
mod golden_tests {
    use super::*;
    use serde_json::Value;

    const GOLDEN_TEXT: &str =
        include_str!("../../../scratch/rust_parity/bibtex_epub_s15/golden_bibtex.json");
    const GOLDEN_BIN: &str =
        include_str!("../../../scratch/rust_parity/bibtex_epub_s15/golden_bibtex_bin.json");

    /// The fourteen string fields the port models, in one place so every
    /// golden case checks the exact same surface.
    const FIELDS: [&str; 14] = [
        "entry_type",
        "cite_key",
        "author",
        "year",
        "date",
        "title",
        "journal",
        "booktitle",
        "publisher",
        "volume",
        "pages",
        "doi",
        "url",
        "short_cite",
    ];

    fn rust_field<'a>(e: &'a BibEntry, f: &str) -> &'a str {
        match f {
            "entry_type" => &e.entry_type,
            "cite_key" => &e.cite_key,
            "author" => &e.author,
            "year" => &e.year,
            "date" => &e.date,
            "title" => &e.title,
            "journal" => &e.journal,
            "booktitle" => &e.booktitle,
            "publisher" => &e.publisher,
            "volume" => &e.volume,
            "pages" => &e.pages,
            "doi" => &e.doi,
            "url" => &e.url,
            "short_cite" => &e.short_cite,
            _ => unreachable!(),
        }
    }

    /// Compare one parsed entry against its CPython golden across every
    /// enumerated field plus `full_reference`, failing with a readable diff.
    fn assert_entry(case: &str, key: &str, got: &BibEntry, exp: &Value) {
        for f in FIELDS.iter() {
            let want = exp[*f].as_str().unwrap_or("<missing>");
            let have = rust_field(got, f);
            assert_eq!(
                have, want,
                "case {case} key {key} field {f}\n  python = {want:?}\n  rust   = {have:?}"
            );
        }
        assert_eq!(
            got.full_reference,
            exp["full_reference"].as_str().unwrap_or(""),
            "case {case} key {key} field full_reference"
        );
    }

    #[test]
    fn bibtex_text_goldens_match_cpython() {
        let cases: Vec<Value> =
            serde_json::from_str(GOLDEN_TEXT).expect("golden_bibtex.json must be a JSON array");
        assert!(!cases.is_empty(), "golden_bibtex.json produced zero cases");
        for c in &cases {
            let case = c["case"].as_str().unwrap_or("?");
            let input = c["input"].as_str().expect("golden input string");
            let exp_entries = c["entries"].as_object().expect("golden entries object");
            let got = parse_bibtex_content(input);

            let mut got_keys: Vec<&String> = got.keys().collect();
            let mut exp_keys: Vec<&String> = exp_entries.keys().collect();
            got_keys.sort();
            exp_keys.sort();
            assert_eq!(
                got_keys, exp_keys,
                "case {case}: cite-key set differs (rust parsed entries that python did not, or vice-versa)"
            );

            for (key, exp) in exp_entries {
                let entry = got
                    .get(key)
                    .unwrap_or_else(|| panic!("case {case}: rust lost key {key:?}"));
                assert_entry(case, key, entry, exp);
            }
        }
    }

    #[test]
    fn bibtex_invalid_utf8_goldens_match_cpython() {
        // Same raw byte literals the generator wrote (gen_goldens.py
        // `gen_bibtex_bin`).  `parse_bibtex_file` must decode them like
        // `open(..., errors='replace')`: one U+FFFD per maximal invalid subpart,
        // then keep parsing instead of bailing on invalid UTF-8.
        let raws: [(&str, &[u8]); 3] = [
            (
                "latin1_e",
                b"@misc{b1, author = {X}, title = {caf\xe9}, year = {2020}\n}",
            ),
            (
                "lone_80",
                b"@misc{b2, author = {X}, title = {a\x80b}, year = {2021}\n}",
            ),
            (
                "truncated",
                b"@misc{b3, author = {X}, title = {na\xefve}\xc3\x28, year = {2022}\n}",
            ),
        ];

        let cases: Vec<Value> =
            serde_json::from_str(GOLDEN_BIN).expect("golden_bibtex_bin.json must be a JSON array");
        assert!(!cases.is_empty(), "golden_bibtex_bin.json produced zero cases");

        let dir = std::env::temp_dir().join(format!("readmd-bibtex-bin-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        for (case, raw) in raws.iter() {
            let want = cases
                .iter()
                .find(|c| c["case"].as_str() == Some(*case))
                .unwrap_or_else(|| panic!("golden_bibtex_bin.json missing case {case}"));
            let path = dir.join(format!("{case}.bib"));
            std::fs::write(&path, raw).expect("write temp bib");
            let got = parse_bibtex_file(&path);
            let exp_entries = want["entries"].as_object().expect("bin entries object");

            let mut got_keys: Vec<&String> = got.keys().collect();
            let mut exp_keys: Vec<&String> = exp_entries.keys().collect();
            got_keys.sort();
            exp_keys.sort();
            assert_eq!(got_keys, exp_keys, "case {case}: cite-key set differs");

            for (key, exp) in exp_entries {
                let entry = got
                    .get(key)
                    .unwrap_or_else(|| panic!("case {case}: rust lost key {key:?}"));
                assert_entry(case, key, entry, exp);
            }
            let _ = std::fs::remove_file(&path);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn date_only_entry_falls_back_for_short_cite() {
        // Regression guard for D1, pinned by the `date_only_no_year` golden.
        let e = parse_bibtex_content("@misc{d1, author = {Smith, J}, date = {2019}, title = {T}\n}");
        let entry = e.get("d1").expect("d1 present");
        assert_eq!(entry.year, "");
        assert_eq!(entry.date, "2019");
        assert_eq!(entry.short_cite, "[Smith, 2019]");
        // `full_reference` must NOT use the date (bibtex.py:172 reads only year).
        assert_eq!(entry.full_reference, "Smith, J. T.");
    }
}
