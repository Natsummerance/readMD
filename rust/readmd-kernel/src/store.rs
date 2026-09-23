//! SQLite metadata index: documents, derived full text, recents, tags, links
//! and AI chat history. Markdown bytes stay on the filesystem; this table is
//! rebuildable derived data.

use crate::error::{Error, Result};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

pub const SCHEMA_VERSION: i64 = 1;
const MAX_TOKENS_PER_DOC: usize = 60_000;
const SNIPPET_RADIUS: usize = 48;
/// Nesting ceiling for [`pyjson::parse`], mirroring CPython's default
/// `sys.getrecursionlimit()` of 1000, less the frames already in use.
///
/// `json.load` runs through the `_json` C scanner, which calls
/// `Py_EnterRecursiveCall` once per container level.  Measured on this box
/// (Python 3.11.15, `scratch/rust_parity/wd10_depth.py`): the limit is 1000 and
/// `json.loads("["*d + "0" + "]"*d)` **accepts d = 995 and raises
/// `RecursionError` at d = 996** when called from a top-level script.  The real
/// ceiling is `recursionlimit` minus however many frames the caller already has
/// on the stack, so it is not a single number — inside `readmd.py`'s handler
/// chain it sits a few levels lower than 995.
///
/// 1000 is therefore deliberately the *generous* end.  The residual
/// disagreement is the narrow band 996..1000, where Python substitutes its
/// `default` and this parser keeps the tree.  Erring generous is the safer
/// direction: anything tighter than ~995 would reject a file Python reads, and
/// because `load_json` falls back to its `default` instead of propagating, the
/// next `save_json` would overwrite the user's settings with that default — data
/// loss, whereas the generous band only ever preserves the user's own bytes.
const MAX_JSON_DEPTH: usize = 1000;

#[derive(Debug, Clone)]
pub struct DocRecord {
    pub id: i64,
    pub path: String,
    pub title: String,
    pub mtime: i64,
    pub size: i64,
    pub words: i64,
    pub kind: String,
    pub pinned: bool,
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub path: String,
    pub title: String,
    pub score: f64,
    pub snippet: String,
    pub mtime: i64,
}

#[derive(Debug, Clone)]
pub struct RecentItem {
    pub path: String,
    pub title: String,
    pub opened_at: i64,
}

#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30ff | 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff | 0xac00..=0xd7af)
}

pub fn is_cjk_char(c: char) -> bool {
    is_cjk(c)
}

fn bump(map: &mut HashMap<String, i32>, token: String, weight: i32) {
    if token.is_empty() || map.len() >= MAX_TOKENS_PER_DOC {
        return;
    }
    let entry = map.entry(token).or_insert(0);
    *entry = entry.saturating_add(weight);
}

/// Mixed CJK/latin tokenizer. Latin words are lowercased; CJK contributes both
/// single characters and adjacent bigrams, which is what makes Chinese documents
/// searchable without a dictionary.
pub fn tokenize(text: &str) -> Vec<(String, i32)> {
    let mut counts: HashMap<String, i32> = HashMap::new();
    let mut word = String::new();
    let mut prev_cjk: Option<char> = None;
    for c in text.chars() {
        if is_cjk(c) {
            if !word.is_empty() {
                bump(&mut counts, std::mem::take(&mut word), 3);
            }
            let lower = c.to_lowercase().collect::<String>();
            bump(&mut counts, lower.clone(), 2);
            if let Some(p) = prev_cjk {
                if is_cjk(p) {
                    let mut pair = p.to_lowercase().collect::<String>();
                    pair.push(c.to_lowercase().next().unwrap_or(c));
                    bump(&mut counts, pair, 4);
                }
            }
            prev_cjk = Some(c);
            continue;
        }
        prev_cjk = None;
        if c.is_alphanumeric() {
            word.extend(c.to_lowercase());
        } else if !word.is_empty() {
            bump(&mut counts, std::mem::take(&mut word), 3);
        }
    }
    if !word.is_empty() {
        bump(&mut counts, word, 3);
    }
    counts.into_iter().collect()
}

pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl Store {
    pub fn open(path: &Path) -> Result<Store> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Store::connect(path.to_string_lossy().as_ref())
    }

    pub fn in_memory() -> Result<Store> {
        Store::connect(":memory:")
    }

    fn connect(target: &str) -> Result<Store> {
        let mut conn = Connection::open(target)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrate(&mut conn)?;
        Ok(Store { conn: Arc::new(Mutex::new(conn)) })
    }

    fn guard(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn upsert_document(
        &self,
        path: &str,
        title: &str,
        mtime: i64,
        size: i64,
        words: i64,
        kind: &str,
        body: &str,
    ) -> Result<i64> {
        let mut conn = self.guard();
        let tx = conn.transaction()?;
        let id: i64 = match tx.query_row(
            "SELECT id FROM docs WHERE path = ?1",
            params![path],
            |r| r.get::<_, i64>(0),
        ) {
            Ok(existing) => {
                tx.execute(
                    "UPDATE docs SET title=?1, mtime=?2, size=?3, words=?4, kind=?5, body=?6, \
                     updated_at=?7 WHERE id=?8",
                    params![title, mtime, size, words, kind, body, now_millis(), existing],
                )?;
                tx.execute("DELETE FROM tokens WHERE doc_id = ?1", params![existing])?;
                existing
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                tx.execute(
                    "INSERT INTO docs (path, title, mtime, size, words, kind, body, updated_at) \
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![path, title, mtime, size, words, kind, body, now_millis()],
                )?;
                tx.last_insert_rowid()
            }
            Err(e) => return Err(e.into()),
        };
        for (token, weight) in tokenize(body) {
            tx.execute(
                "INSERT INTO tokens (doc_id, token, weight) VALUES (?1,?2,?3) \
                 ON CONFLICT(doc_id, token) DO UPDATE SET weight = weight + excluded.weight",
                params![id, token, weight],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }

    pub fn delete_document(&self, path: &str) -> Result<()> {
        let conn = self.guard();
        if let Ok(id) = conn.query_row("SELECT id FROM docs WHERE path=?1", params![path], |r| {
            r.get::<_, i64>(0)
        }) {
            conn.execute("DELETE FROM tokens WHERE doc_id=?1", params![id])?;
        }
        conn.execute("DELETE FROM docs WHERE path=?1", params![path])?;
        conn.execute("DELETE FROM recent WHERE path=?1", params![path])?;
        conn.execute("DELETE FROM links WHERE src=?1 OR dst=?1", params![path])?;
        Ok(())
    }

    /// Move a document's index entry to a new path.
    ///
    /// A rename can land on a path the index still believes exists — a stale
    /// `docs` row for a file that went away without a reindex, a case-only
    /// rename on Windows, or a link target that was already recorded.  `docs.path`
    /// is `UNIQUE` and `links`' primary key is `(src, dst)`, so the plain
    /// `UPDATE`s this function used to run abort with
    /// `UNIQUE constraint failed: docs.path` there.  The caller cannot survive
    /// that: server.rs's back-reference closure propagates with `?`, so
    /// `content::index(app, &new_path)` is skipped and the index is left holding
    /// a row for a path that no longer exists *and* no row for the one that does.
    ///
    /// Python cannot fail this way.  `Api.rename_file`'s sync pass
    /// (`readmd.py:4794-4802`) rebuilds the JSON lists with an
    /// "keep the first occurrence, drop the equal one" pass, so a collision is
    /// just a dedupe.  Each statement below is therefore collision-tolerant, and
    /// the whole move runs in one transaction so a partially applied rename is
    /// never observable.
    pub fn rename_document(&self, from: &str, to: &str) -> Result<()> {
        // A no-op rename must stay a no-op: without this guard the "drop the
        // stale occupant of `to`" step below would delete the very row being
        // renamed.  Python lands on the same result by construction, since
        // `_paths_equal(item, old_path)` rewrites the entry to itself and the
        // keep-first dedupe leaves exactly one copy
        // (`readmd.py:4796-4800`).
        if from == to {
            return Ok(());
        }
        let mut conn = self.guard();
        let tx = conn.transaction()?;
        // The file that now exists at `to` wins over a stale index entry.
        let stale: Option<i64> = tx
            .query_row("SELECT id FROM docs WHERE path=?1", params![to], |r| {
                r.get(0)
            })
            .ok();
        if let Some(id) = stale {
            tx.execute("DELETE FROM tokens WHERE doc_id=?1", params![id])?;
            tx.execute("DELETE FROM doc_tags WHERE doc_id=?1", params![id])?;
            tx.execute("DELETE FROM docs WHERE id=?1", params![id])?;
        }
        tx.execute("UPDATE docs SET path=?1 WHERE path=?2", params![to, from])?;
        // `UPDATE OR IGNORE` followed by a `DELETE` of the old key is the SQL
        // spelling of Python's keep-first dedupe: a row that could not move
        // because the target already existed is dropped.  `recent.path` is a
        // primary key, so it needs the same treatment.
        tx.execute(
            "UPDATE OR IGNORE recent SET path=?1 WHERE path=?2",
            params![to, from],
        )?;
        tx.execute("DELETE FROM recent WHERE path=?1", params![from])?;
        tx.execute(
            "UPDATE OR IGNORE links SET src=?1 WHERE src=?2",
            params![to, from],
        )?;
        tx.execute("DELETE FROM links WHERE src=?1", params![from])?;
        tx.execute(
            "UPDATE OR IGNORE links SET dst=?1 WHERE dst=?2",
            params![to, from],
        )?;
        tx.execute("DELETE FROM links WHERE dst=?1", params![from])?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_pinned(&self, path: &str, pinned: bool) -> Result<()> {
        let conn = self.guard();
        conn.execute(
            "UPDATE docs SET pinned=?1 WHERE path=?2",
            params![pinned as i64, path],
        )?;
        Ok(())
    }

    pub fn get_doc(&self, path: &str) -> Result<Option<DocRecord>> {
        let conn = self.guard();
        let mut stmt = conn.prepare(
            "SELECT id, path, title, mtime, size, words, kind, pinned FROM docs WHERE path=?1",
        )?;
        let row = stmt
            .query_row(params![path], row_to_doc)
            .ok();
        Ok(row)
    }

    pub fn list_documents(&self, limit: usize, kind: Option<&str>) -> Result<Vec<DocRecord>> {
        let conn = self.guard();
        let sql = match kind {
            Some(_) => "SELECT id, path, title, mtime, size, words, kind, pinned FROM docs \
                        WHERE kind=?1 ORDER BY pinned DESC, mtime DESC LIMIT ?2",
            None => "SELECT id, path, title, mtime, size, words, kind, pinned FROM docs \
                     ORDER BY pinned DESC, mtime DESC LIMIT ?1",
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = match kind {
            Some(k) => stmt.query_map(params![k, limit as i64], row_to_doc)?,
            None => stmt.query_map(params![limit as i64], row_to_doc)?,
        };
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Ranked search over the derived token index. Query tokens are combined
    /// additively with an idf-like penalty so rare tokens dominate.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        let mut tokens: Vec<(String, i32)> = tokenize(query);
        // `tokenize` returns `HashMap` order, so truncating it directly would
        // keep an *arbitrary* 12 distinct tokens and make the hit set vary
        // between two identical calls.  Python's index queries always carry a
        // total `ORDER BY` (see `link_indexer.py:354`, `:389`), so rank the
        // tokens first: heaviest weight wins, ties broken by token text.
        tokens.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        tokens.truncate(12);
        if tokens.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.guard();
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM docs", [], |r| r.get(0))
            .unwrap_or(1)
            .max(1);
        let mut scores: HashMap<i64, f64> = HashMap::new();
        let mut bodies: HashMap<i64, (String, String, i64, String)> = HashMap::new();
        for (token, qweight) in &tokens {
            let df: i64 = conn
                .query_row("SELECT COUNT(DISTINCT doc_id) FROM tokens WHERE token=?1", params![token], |r| r.get(0))
                .unwrap_or(0);
            if df == 0 {
                continue;
            }
            let idf = ((total as f64 + 1.0) / (df as f64 + 1.0)).ln() + 1.0;
            let mut stmt = conn.prepare(
                "SELECT d.id, d.path, d.title, d.mtime, t.weight FROM tokens t \
                 JOIN docs d ON d.id = t.doc_id WHERE t.token=?1 LIMIT 400",
            )?;
            let rows = stmt.query_map(params![token], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })?;
            for row in rows.flatten() {
                *scores.entry(row.0).or_insert(0.0) += (*qweight as f64) * (row.4 as f64) * idf;
                bodies.entry(row.0).or_insert((row.1, row.2, row.3, String::new()));
            }
        }
        let mut ranked: Vec<(String, String, i64, f64)> = Vec::with_capacity(bodies.len());
        for (id, entry) in bodies.iter() {
            match scores.get(id) {
                Some(score) => ranked.push((entry.0.clone(), entry.1.clone(), entry.2, *score)),
                None => continue,
            }
        }
        // A total order: score, then recency, then path.  Sorting on score alone
        // left ties in `HashMap` order, so two identical searches could return
        // the same hits in a different sequence.
        ranked.sort_by(|a, b| {
            b.3.partial_cmp(&a.3)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.2.cmp(&a.2))
                .then_with(|| a.0.cmp(&b.0))
        });
        let mut out = Vec::new();
        for (path, title, mtime, score) in ranked.into_iter().take(limit) {
            let body: String = conn
                .query_row(
                    "SELECT body FROM docs WHERE path=?1",
                    params![path.as_str()],
                    |r| r.get::<_, Option<String>>(0),
                )
                .ok()
                .flatten()
                .unwrap_or_default();
            out.push(SearchHit {
                snippet: make_snippet(&body, &tokens),
                score,
                path,
                title,
                mtime,
            });
        }
        Ok(out)
    }

    pub fn touch_recent(&self, path: &str, title: &str) -> Result<()> {
        let conn = self.guard();
        conn.execute(
            "INSERT INTO recent (path, title, opened_at) VALUES (?1,?2,?3) \
             ON CONFLICT(path) DO UPDATE SET title=excluded.title, opened_at=excluded.opened_at",
            params![path, title, now_millis()],
        )?;
        Ok(())
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<RecentItem>> {
        let conn = self.guard();
        let mut stmt =
            conn.prepare("SELECT path, title, opened_at FROM recent ORDER BY opened_at DESC LIMIT ?1")?;
        let rows = stmt.query_map(params![limit as i64], |r| {
            Ok(RecentItem {
                path: r.get(0)?,
                title: r.get(1)?,
                opened_at: r.get(2)?,
            })
        })?;
        Ok(rows.flatten().collect())
    }

    pub fn remove_recent(&self, path: &str) -> Result<()> {
        let conn = self.guard();
        conn.execute("DELETE FROM recent WHERE path=?1", params![path])?;
        Ok(())
    }

    pub fn clear_recent(&self) -> Result<()> {
        let conn = self.guard();
        conn.execute("DELETE FROM recent", [])?;
        Ok(())
    }

    pub fn replace_links(&self, src: &str, targets: &[String]) -> Result<()> {
        let mut conn = self.guard();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM links WHERE src=?1", params![src])?;
        let mut seen: HashSet<String> = HashSet::new();
        for dst in targets {
            if dst == src || !seen.insert(dst.clone()) {
                continue;
            }
            tx.execute("INSERT INTO links (src, dst) VALUES (?1,?2)", params![src, dst])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn backlinks(&self, target: &str) -> Result<Vec<String>> {
        let conn = self.guard();
        let mut stmt = conn.prepare("SELECT src FROM links WHERE dst=?1 ORDER BY src")?;
        let rows = stmt.query_map(params![target], |r| r.get::<_, String>(0))?;
        Ok(rows.flatten().collect())
    }

    pub fn graph(&self) -> Result<Value> {
        let conn = self.guard();
        let mut nodes: Vec<Value> = Vec::new();
        {
            let mut stmt = conn.prepare("SELECT path, title FROM docs ORDER BY path LIMIT 2000")?;
            let rows = stmt.query_map([], |r| {
                Ok(json!({ "id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)? }))
            })?;
            for row in rows.flatten() {
                nodes.push(row);
            }
        }
        let mut edges: Vec<Value> = Vec::new();
        {
            let mut stmt = conn.prepare("SELECT src, dst FROM links ORDER BY src LIMIT 8000")?;
            let rows = stmt.query_map([], |r| {
                Ok(json!({ "source": r.get::<_, String>(0)?, "target": r.get::<_, String>(1)? }))
            })?;
            for row in rows.flatten() {
                edges.push(row);
            }
        }
        Ok(json!({ "nodes": nodes, "edges": edges }))
    }

    /// Links whose target is missing from the indexed corpus.
    pub fn deadlinks(&self) -> Result<Vec<Value>> {
        let conn = self.guard();
        let mut stmt = conn.prepare(
            "SELECT l.src, l.dst FROM links l LEFT JOIN docs d ON d.path = l.dst \
             WHERE d.id IS NULL ORDER BY l.src LIMIT 500",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(json!({ "source": r.get::<_, String>(0)?, "target": r.get::<_, String>(1)? }))
        })?;
        Ok(rows.flatten().collect())
    }

    pub fn remember_chat(&self, session: &str, role: &str, content: &str, model: &str) -> Result<()> {
        let conn = self.guard();
        conn.execute(
            "INSERT INTO chat_messages (session, role, content, model, created_at) \
             VALUES (?1,?2,?3,?4,?5)",
            params![session, role, content, model, now_millis()],
        )?;
        Ok(())
    }

    pub fn chat_history(&self, session: &str, limit: usize) -> Result<Vec<Value>> {
        let conn = self.guard();
        let mut stmt = conn.prepare(
            "SELECT role, content, model, created_at FROM chat_messages WHERE session=?1 \
             ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![session, limit as i64], |r| {
            Ok(json!({
                "role": r.get::<_, String>(0)?,
                "content": r.get::<_, String>(1)?,
                "model": r.get::<_, String>(2)?,
                "time": r.get::<_, i64>(3)?,
            }))
        })?;
        let mut out: Vec<Value> = rows.flatten().collect();
        out.reverse();
        Ok(out)
    }

    pub fn clear_chat(&self, session: &str) -> Result<()> {
        let conn = self.guard();
        if session.is_empty() {
            conn.execute("DELETE FROM chat_messages", [])?;
        } else {
            conn.execute("DELETE FROM chat_messages WHERE session=?1", params![session])?;
        }
        Ok(())
    }

    pub fn stats(&self) -> Result<Value> {
        let conn = self.guard();
        let docs: i64 = conn.query_row("SELECT COUNT(*) FROM docs", [], |r| r.get(0)).unwrap_or(0);
        let tokens: i64 = conn
            .query_row("SELECT COUNT(*) FROM tokens", [], |r| r.get(0))
            .unwrap_or(0);
        let links: i64 = conn
            .query_row("SELECT COUNT(*) FROM links", [], |r| r.get(0))
            .unwrap_or(0);
        let words: i64 = conn
            .query_row("SELECT COALESCE(SUM(words),0) FROM docs", [], |r| r.get(0))
            .unwrap_or(0);
        Ok(json!({ "docs": docs, "tokens": tokens, "links": links, "words": words }))
    }
}

fn row_to_doc(r: &rusqlite::Row<'_>) -> rusqlite::Result<DocRecord> {
    Ok(DocRecord {
        id: r.get(0)?,
        path: r.get(1)?,
        title: r.get(2)?,
        mtime: r.get(3)?,
        size: r.get(4)?,
        words: r.get(5)?,
        kind: r.get(6)?,
        pinned: r.get::<_, i64>(7)? != 0,
    })
}

fn make_snippet(body: &str, tokens: &[(String, i32)]) -> String {
    let chars: Vec<char> = body.chars().collect();
    // Full lowercasing is *not* length preserving: `İ` (U+0130) folds to
    // `i` + COMBINING DOT ABOVE, i.e. one source char becomes two lowered
    // chars.  Building the match text as one flat string and then using a
    // character count into it as an index into `chars` therefore produced an
    // anchor past the end of `chars`, which made `chars[start..end]` panic with
    // "range ends before begin" for any document whose case-folding expanded
    // before the match.  `origin[i]` keeps the lowered-char → source-char
    // mapping explicit, so the anchor is always expressed in `chars` units.
    let mut lower = String::new();
    let mut origin: Vec<usize> = Vec::with_capacity(chars.len());
    for (i, c) in chars.iter().enumerate() {
        for l in c.to_lowercase() {
            lower.push(l);
            origin.push(i);
        }
    }
    let mut best: Option<usize> = None;
    if !origin.is_empty() {
        for (token, _) in tokens {
            if token.is_empty() {
                continue;
            }
            if let Some(pos) = lower.find(token.as_str()) {
                let lowered_idx = lower[..pos].chars().count();
                let idx = origin[lowered_idx.min(origin.len() - 1)];
                best = Some(match best {
                    Some(b) => b.min(idx),
                    None => idx,
                });
            }
        }
    }
    let start = best.unwrap_or(0).saturating_sub(SNIPPET_RADIUS / 2);
    let end = (start + SNIPPET_RADIUS * 2).min(chars.len());
    if start >= end {
        return String::new();
    }
    let mut out: String = chars[start..end].iter().collect();
    if start > 0 {
        out = format!("...{}", out);
    }
    if end < chars.len() {
        out.push_str("...");
    }
    out
}

/// Non-key columns that [`migrate`] may add to a table an older kernel build
/// already created.  Each declaration is the same text the canonical
/// `CREATE TABLE` below uses, so a backfilled column is indistinguishable from
/// one that was present from the start.
///
/// Only columns that carry a `DEFAULT` qualify: SQLite refuses
/// `ADD COLUMN … NOT NULL` without one on a populated table.  The key and
/// `NOT NULL`-without-default columns are checked separately against
/// [`REQUIRED_COLUMNS`].
const ADDABLE_COLUMNS: &[(&str, &str, &str)] = &[
    ("docs", "title", "TEXT NOT NULL DEFAULT ''"),
    ("docs", "mtime", "INTEGER NOT NULL DEFAULT 0"),
    ("docs", "size", "INTEGER NOT NULL DEFAULT 0"),
    ("docs", "words", "INTEGER NOT NULL DEFAULT 0"),
    ("docs", "kind", "TEXT NOT NULL DEFAULT 'md'"),
    ("docs", "pinned", "INTEGER NOT NULL DEFAULT 0"),
    ("docs", "body", "TEXT NOT NULL DEFAULT ''"),
    ("docs", "updated_at", "INTEGER NOT NULL DEFAULT 0"),
    ("tokens", "weight", "INTEGER NOT NULL DEFAULT 1"),
    ("recent", "title", "TEXT NOT NULL DEFAULT ''"),
    ("recent", "opened_at", "INTEGER NOT NULL DEFAULT 0"),
    ("chat_messages", "model", "TEXT NOT NULL DEFAULT ''"),
    ("chat_messages", "created_at", "INTEGER NOT NULL DEFAULT 0"),
];

/// Columns that cannot be repaired in place because they are a key or a
/// `NOT NULL` column without a default.  If one is missing the file is not a
/// `readmd.db` this build can read, and `Store::open` says so instead of
/// returning a handle whose every write would fail.
const REQUIRED_COLUMNS: &[(&str, &str)] = &[
    ("meta", "key"),
    ("meta", "value"),
    ("docs", "id"),
    ("docs", "path"),
    ("tokens", "doc_id"),
    ("tokens", "token"),
    ("recent", "path"),
    ("links", "src"),
    ("links", "dst"),
    ("tags", "id"),
    ("tags", "name"),
    ("doc_tags", "doc_id"),
    ("doc_tags", "tag_id"),
    ("chat_messages", "id"),
    ("chat_messages", "session"),
    ("chat_messages", "role"),
    ("chat_messages", "content"),
];

/// Live column names of `table`, or an empty set when the table does not exist.
fn columns_of(conn: &Connection, table: &str) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare("SELECT name FROM pragma_table_info(?1)")?;
    let rows = stmt.query_map(params![table], |r| r.get::<_, String>(0))?;
    let mut out = HashSet::new();
    for name in rows {
        out.insert(name?.to_lowercase());
    }
    Ok(out)
}

/// The canonical table set, kept separate from the index set so that columns
/// can be backfilled before an index that references them is created.
const CREATE_TABLES: &str = "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS docs (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL DEFAULT '',
            mtime INTEGER NOT NULL DEFAULT 0,
            size INTEGER NOT NULL DEFAULT 0,
            words INTEGER NOT NULL DEFAULT 0,
            kind TEXT NOT NULL DEFAULT 'md',
            pinned INTEGER NOT NULL DEFAULT 0,
            body TEXT NOT NULL DEFAULT '',
            updated_at INTEGER NOT NULL DEFAULT 0);
         CREATE TABLE IF NOT EXISTS tokens (
            doc_id INTEGER NOT NULL,
            token TEXT NOT NULL,
            weight INTEGER NOT NULL DEFAULT 1,
            PRIMARY KEY (doc_id, token));
         CREATE TABLE IF NOT EXISTS recent (
            path TEXT PRIMARY KEY,
            title TEXT NOT NULL DEFAULT '',
            opened_at INTEGER NOT NULL DEFAULT 0);
         CREATE TABLE IF NOT EXISTS links (
            src TEXT NOT NULL,
            dst TEXT NOT NULL,
            PRIMARY KEY (src, dst));
         CREATE TABLE IF NOT EXISTS tags (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE);
         CREATE TABLE IF NOT EXISTS doc_tags (
            doc_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            PRIMARY KEY (doc_id, tag_id));
         CREATE TABLE IF NOT EXISTS chat_messages (
            id INTEGER PRIMARY KEY,
            session TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            model TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL DEFAULT 0);";

const CREATE_INDEXES: &str = "CREATE INDEX IF NOT EXISTS docs_mtime ON docs(mtime DESC);
         CREATE INDEX IF NOT EXISTS docs_kind ON docs(kind);
         CREATE INDEX IF NOT EXISTS tokens_token ON tokens(token);
         CREATE INDEX IF NOT EXISTS chat_session ON chat_messages(session, id);";

fn migrate(conn: &mut Connection) -> Result<()> {
    // 1. Tables, without their indexes.
    conn.execute_batch(CREATE_TABLES)?;

    // 2. Backfill columns a `CREATE TABLE IF NOT EXISTS` could never add.  One
    //    `pragma_table_info` read per table rather than per column.
    let mut have: HashMap<&'static str, HashSet<String>> = HashMap::new();
    for (table, column, decl) in ADDABLE_COLUMNS {
        if !have.contains_key(*table) {
            let fetched = columns_of(conn, table)?;
            have.insert(table, fetched);
        }
        let set = have.get_mut(*table).expect("populated just above");
        if set.contains(*column) {
            continue;
        }
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"),
            [],
        )?;
        set.insert(column.to_lowercase());
    }

    // 3. Indexes, now that every indexed column is guaranteed present.
    conn.execute_batch(CREATE_INDEXES)?;

    // 4. A missing key column means this is not a database this build can
    //    write to; fail loudly at open time instead of on the first request.
    for (table, column) in REQUIRED_COLUMNS {
        let have = columns_of(conn, table)?;
        if !have.contains(&column.to_lowercase()) {
            return Err(Error::Msg(format!("db_schema_too_old: {table}.{column}")));
        }
    }

    let stored: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key='schema_version'", [], |r| {
            r.get(0)
        })
        .ok();
    if let Some(raw) = &stored {
        // A file written by a newer kernel must not be relabelled as this one's
        // version: the columns it relies on would then look "up to date".
        let parsed: i64 = raw.trim().parse().unwrap_or(SCHEMA_VERSION);
        if parsed > SCHEMA_VERSION {
            return Err(Error::Msg(format!(
                "db_schema_too_new: found {raw}, this kernel supports {SCHEMA_VERSION}"
            )));
        }
        if raw.trim() == SCHEMA_VERSION.to_string() {
            return Ok(());
        }
    }
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('schema_version', ?1) \
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![SCHEMA_VERSION.to_string()],
    )?;
    Ok(())
}

// =============================================== on-disk JSON (Python parity)
//
// `readmd.py` persists its settings and recent list through
// `src/readmd_core/utils.py: save_json`, i.e.
// `json.dump(data, f, ensure_ascii=False, indent=2)` written to a unique
// sibling temp file and then `os.replace`d with six short retries.  Two
// properties of that call are load-bearing for byte parity and cannot be
// reproduced with the pinned `serde_json` (no `preserve_order`, so
// `Value::Object` is a `BTreeMap` and reorders every key): object keys keep
// *insertion* order, and non-ASCII text is emitted raw while `0x7f` stays raw
// as well.  [`pyjson::Pj`] is therefore the representation used for anything
// that touches disk.  HTTP response bodies keep using `serde_json::Value`
// because the parity harness compares object keys as a sorted set.

pub use pyjson::Pj;

/// Basename of `RECENT_FILE` (`src/readmd_core/config.py:48`, imported into
/// `readmd.py:84`).  Only the basename is kept here because `DATA_DIR` is
/// passed in by the caller as `data_dir`.
pub const RECENT_FILE: &str = "recent.json";
/// Basename of `SETTINGS_FILE` (`src/readmd_core/config.py:47`, imported into
/// `readmd.py:83`).  Neither is an `Api` attribute; both are module constants.
pub const SETTINGS_FILE: &str = "settings.json";
/// `add_recent` truncates with `rec[:20]` (`readmd.py:5808`).
pub const RECENT_KEEP: usize = 20;
/// `Api.MAX_RECENT_ENTRIES` (`readmd.py:3709`).
pub const MAX_RECENT_ENTRIES: usize = 24;
/// `Api.MAX_RECENT_PATH_LENGTH` (`readmd.py:3710`).
pub const MAX_RECENT_PATH_LENGTH: usize = 4096;
/// `Api.MAX_RECENT_SCAN_ENTRIES` (`readmd.py:3711`).
pub const MAX_RECENT_SCAN_ENTRIES: usize = 512;
/// `find_in(..., include_children=True)` only descends into 64 children.
pub const MAX_RECENT_CHILD_DIRS: usize = 64;

pub mod pyjson {
    //! Minimal order-preserving JSON codec matching CPython's `json` module.

    use serde_json::{Map, Value};

    /// A JSON value that remembers object key order and integer literals.
    #[derive(Debug, Clone, PartialEq)]
    pub enum Pj {
        Null,
        Bool(bool),
        /// Kept verbatim because Python ints are unbounded (`1e20` style
        /// reformatting would otherwise corrupt them).
        Int(String),
        Float(f64),
        Str(String),
        Arr(Vec<Pj>),
        Obj(Vec<(String, Pj)>),
    }

    impl Pj {
        pub fn empty_obj() -> Pj {
            Pj::Obj(Vec::new())
        }
        pub fn empty_arr() -> Pj {
            Pj::Arr(Vec::new())
        }
        pub fn is_obj(&self) -> bool {
            matches!(self, Pj::Obj(_))
        }
        pub fn as_str(&self) -> Option<&str> {
            match self {
                Pj::Str(s) => Some(s.as_str()),
                _ => None,
            }
        }
        pub fn as_array(&self) -> Option<&[Pj]> {
            match self {
                Pj::Arr(v) => Some(v.as_slice()),
                _ => None,
            }
        }
        pub fn get(&self, key: &str) -> Option<&Pj> {
            match self {
                Pj::Obj(fields) => fields.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
                _ => None,
            }
        }
        /// `bool(x)` as Python would evaluate it.
        pub fn truthy(&self) -> bool {
            match self {
                Pj::Null => false,
                Pj::Bool(b) => *b,
                Pj::Int(s) => s.trim_start_matches('-').trim_start_matches('0') != "",
                Pj::Float(f) => *f != 0.0,
                Pj::Str(s) => !s.is_empty(),
                Pj::Arr(v) => !v.is_empty(),
                Pj::Obj(v) => !v.is_empty(),
            }
        }
        /// Lossy bridge into `serde_json` for building response bodies.
        ///
        /// Two directions lose information, both because `serde_json::Value`
        /// is narrower than CPython's `json`:
        /// * an `Int` outside `i64` becomes a JSON **string** — Python writes
        ///   `12345678901234567890` as a bare number, and `Value` can only hold
        ///   that with the `arbitrary_precision` feature, which this crate does
        ///   not enable.
        /// * `NaN` / `Infinity` / `-Infinity` become **null** — Python writes
        ///   those literals verbatim (`parse_constant`), `Number` has no
        ///   non-finite case.
        ///
        /// Use [`to_compact`] / [`to_pretty`] for anything that must match
        /// Python byte-for-byte; those go through this module's own emitter.
        ///
        /// The walk is iterative for the same reason [`emit`] is: the tree being
        /// converted may be [`MAX_JSON_DEPTH`] levels deep, which the caller's
        /// stack cannot reliably carry.
        pub fn to_value(&self) -> Value {
            enum Task<'a> {
                Visit(&'a Pj),
                /// Close a container over the `count` values its children pushed.
                Close { count: usize, keys: Option<Vec<String>> },
            }
            let mut tasks: Vec<Task> = Vec::new();
            let mut done: Vec<Value> = Vec::new();
            tasks.push(Task::Visit(self));
            while let Some(task) = tasks.pop() {
                match task {
                    Task::Visit(v) => match v {
                        Pj::Null => done.push(Value::Null),
                        Pj::Bool(b) => done.push(Value::Bool(*b)),
                        Pj::Int(s) => done.push(
                            s.parse::<i64>()
                                .map(|n| Value::from(n))
                                .unwrap_or_else(|_| Value::String(s.clone())),
                        ),
                        Pj::Float(f) => done.push(
                            serde_json::Number::from_f64(*f)
                                .map(Value::Number)
                                .unwrap_or(Value::Null),
                        ),
                        Pj::Str(s) => done.push(Value::String(s.clone())),
                        Pj::Arr(items) => {
                            tasks.push(Task::Close { count: items.len(), keys: None });
                            for item in items.iter().rev() {
                                tasks.push(Task::Visit(item));
                            }
                        }
                        Pj::Obj(fields) => {
                            tasks.push(Task::Close {
                                count: fields.len(),
                                keys: Some(fields.iter().map(|(k, _)| k.clone()).collect()),
                            });
                            for (_, value) in fields.iter().rev() {
                                tasks.push(Task::Visit(value));
                            }
                        }
                    },
                    Task::Close { count, keys } => {
                        // Every child `Visit` is pushed after its own `Close`, so
                        // exactly `count` values are pending here; `saturating_sub`
                        // keeps that invariant free of any panic path.
                        let start = done.len().saturating_sub(count);
                        let vals: Vec<Value> = done.drain(start..).collect();
                        done.push(match keys {
                            None => Value::Array(vals),
                            Some(keys) => {
                                let mut map = Map::new();
                                for (k, value) in keys.into_iter().zip(vals) {
                                    map.insert(k, value);
                                }
                                Value::Object(map)
                            }
                        });
                    }
                }
            }
            done.pop().unwrap_or(Value::Null)
        }
        /// Bridge out of `serde_json`.  Object key order is whatever
        /// `serde_json`'s `BTreeMap` yields, so this direction is only used to
        /// *read* request bodies, never to write Python-shaped files.
        pub fn from_value(v: &Value) -> Pj {
            match v {
                Value::Null => Pj::Null,
                Value::Bool(b) => Pj::Bool(*b),
                Value::Number(n) => {
                    if n.is_i64() || n.is_u64() {
                        Pj::Int(n.to_string())
                    } else {
                        Pj::Float(n.as_f64().unwrap_or(0.0))
                    }
                }
                Value::String(s) => Pj::Str(s.clone()),
                Value::Array(a) => Pj::Arr(a.iter().map(Pj::from_value).collect()),
                Value::Object(o) => {
                    Pj::Obj(o.iter().map(|(k, v)| (k.clone(), Pj::from_value(v))).collect())
                }
            }
        }
    }

    // ------------------------------------------------------------------ emit

    fn encode_str(s: &str, out: &mut String) {
        out.push('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\u{8}' => out.push_str("\\b"),
                '\t' => out.push_str("\\t"),
                '\n' => out.push_str("\\n"),
                '\u{c}' => out.push_str("\\f"),
                '\r' => out.push_str("\\r"),
                other if (other as u32) < 0x20 => {
                    out.push_str(&format!("\\u{:04x}", other as u32));
                }
                // `ensure_ascii=False` keeps 0x7f and all non-ASCII raw, which
                // is what the measured Python fixture shows.
                other => out.push(other),
            }
        }
        out.push('"');
    }

    /// `repr()` of a float, the way `json.dump` emits it.
    pub fn float_repr(v: f64) -> String {
        if v.is_nan() {
            return "NaN".to_string();
        }
        if v.is_infinite() {
            return if v > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() };
        }
        let negative = v.is_sign_negative();
        let abs = v.abs();
        if abs == 0.0 {
            return if negative { "-0.0".to_string() } else { "0.0".to_string() };
        }
        // Rust's exponential form uses the shortest round-tripping digits, the
        // same digit set CPython's `repr` picks.
        let sci = format!("{:e}", abs);
        let (mantissa, exp) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
        let exp: i32 = exp.parse().unwrap_or(0);
        let digits_raw: String = mantissa.chars().filter(|c| *c != '.').collect();
        let digits = digits_raw.trim_end_matches('0').to_string();
        let digits = if digits.is_empty() { "0".to_string() } else { digits };
        let decpt = exp + 1; // digits that would sit left of the decimal point
        let mut out = String::new();
        if decpt <= -4 || decpt > 16 {
            out.push_str(&digits[..1]);
            if digits.len() > 1 {
                out.push('.');
                out.push_str(&digits[1..]);
            }
            let e = decpt - 1;
            out.push('e');
            out.push(if e < 0 { '-' } else { '+' });
            out.push_str(&format!("{:02}", e.abs()));
        } else if decpt <= 0 {
            out.push_str("0.");
            for _ in 0..(-decpt) {
                out.push('0');
            }
            out.push_str(&digits);
        } else if decpt as usize >= digits.len() {
            out.push_str(&digits);
            for _ in 0..(decpt as usize - digits.len()) {
                out.push('0');
            }
            out.push_str(".0");
        } else {
            let split = decpt as usize;
            out.push_str(&digits[..split]);
            out.push('.');
            out.push_str(&digits[split..]);
        }
        if negative {
            out.insert(0, '-');
        }
        out
    }

    /// Depth is carried by an explicit heap worklist, never by the call stack.
    ///
    /// `pyjson::parse` accepts 1000 nested containers by design (see
    /// [`super::MAX_JSON_DEPTH`]), and the recursive emitter this replaces
    /// measured ~400-650 KiB of caller stack for such a document in a debug
    /// build — on a binary whose main thread has a 1 MiB stack *reserve*, out of
    /// which the tao/wry dispatch chain has already spent frames.  A document
    /// Python would happily write would therefore abort the process here, which
    /// is why there is no depth cap on this path: an iterative walk accepts
    /// exactly what the parse gate accepts.
    enum Job<'a> {
        /// Emit this value; containers expand into more jobs.
        Val(&'a Pj, usize),
        /// Structural punctuation, pushed verbatim.
        Raw(&'static str),
        /// An object key, `encode_str`-escaped at emit time.
        Key(&'a str),
        /// A line break plus `indent * level` spaces; a no-op when compact.
        Nl(usize),
    }

    fn emit(v: &Pj, out: &mut String, level: usize, indent: Option<usize>) {
        // `,` with an indent (Python's `json.dumps` drops the space after the
        // item separator when `indent` is set), `", "` compact.
        let sep = if indent.is_some() { "," } else { ", " };
        let mut jobs: Vec<Job> = Vec::new();
        jobs.push(Job::Val(v, level));
        while let Some(job) = jobs.pop() {
            match job {
                Job::Raw(s) => out.push_str(s),
                Job::Key(k) => encode_str(k, out),
                Job::Nl(depth) => {
                    if let Some(width) = indent {
                        out.push('\n');
                        for _ in 0..(width * depth) {
                            out.push(' ');
                        }
                    }
                }
                Job::Val(v, level) => match v {
                    Pj::Null => out.push_str("null"),
                    Pj::Bool(true) => out.push_str("true"),
                    Pj::Bool(false) => out.push_str("false"),
                    Pj::Int(s) => out.push_str(s),
                    Pj::Float(f) => out.push_str(&float_repr(*f)),
                    Pj::Str(s) => encode_str(s, out),
                    Pj::Arr(items) => {
                        if items.is_empty() {
                            out.push_str("[]");
                            continue;
                        }
                        out.push('[');
                        // Last thing emitted is pushed first: `newline_at(level)`
                        // then `]`, then every entry in reverse.
                        jobs.push(Job::Raw("]"));
                        jobs.push(Job::Nl(level));
                        for (i, item) in items.iter().enumerate().rev() {
                            jobs.push(Job::Val(item, level + 1));
                            if indent.is_some() {
                                jobs.push(Job::Nl(level + 1));
                            }
                            if i > 0 {
                                jobs.push(Job::Raw(sep));
                            }
                        }
                    }
                    Pj::Obj(fields) => {
                        if fields.is_empty() {
                            out.push_str("{}");
                            continue;
                        }
                        out.push('{');
                        jobs.push(Job::Raw("}"));
                        jobs.push(Job::Nl(level));
                        for (i, (k, value)) in fields.iter().enumerate().rev() {
                            jobs.push(Job::Val(value, level + 1));
                            jobs.push(Job::Raw(": "));
                            jobs.push(Job::Key(k));
                            if indent.is_some() {
                                jobs.push(Job::Nl(level + 1));
                            }
                            if i > 0 {
                                jobs.push(Job::Raw(sep));
                            }
                        }
                    }
                },
            }
        }
    }

    /// The verbatim recursive implementations that the two iterative rewrites
    /// above replaced, kept under `cfg(test)` as a differential oracle: the suite
    /// asserts the iterative `emit` / `Pj::to_value` produce byte-identical
    /// results, so the rewrite provably changed no output.
    #[cfg(test)]
    pub(crate) mod reference {
        use super::{encode_str, float_repr, Pj};
        use serde_json::{Map, Value};

        pub fn emit(v: &Pj, out: &mut String, level: usize, indent: Option<usize>) {
            let newline_at = |out: &mut String, depth: usize| {
                if let Some(width) = indent {
                    out.push('\n');
                    for _ in 0..(width * depth) {
                        out.push(' ');
                    }
                }
            };
            match v {
                Pj::Null => out.push_str("null"),
                Pj::Bool(true) => out.push_str("true"),
                Pj::Bool(false) => out.push_str("false"),
                Pj::Int(s) => out.push_str(s),
                Pj::Float(f) => out.push_str(&float_repr(*f)),
                Pj::Str(s) => encode_str(s, out),
                Pj::Arr(items) => {
                    if items.is_empty() {
                        out.push_str("[]");
                        return;
                    }
                    out.push('[');
                    for (i, item) in items.iter().enumerate() {
                        if i > 0 {
                            if indent.is_some() {
                                out.push(',');
                            } else {
                                out.push_str(", ");
                            }
                        }
                        newline_at(out, level + 1);
                        emit(item, out, level + 1, indent);
                    }
                    newline_at(out, level);
                    out.push(']');
                }
                Pj::Obj(fields) => {
                    if fields.is_empty() {
                        out.push_str("{}");
                        return;
                    }
                    out.push('{');
                    for (i, (k, value)) in fields.iter().enumerate() {
                        if i > 0 {
                            if indent.is_some() {
                                out.push(',');
                            } else {
                                out.push_str(", ");
                            }
                        }
                        newline_at(out, level + 1);
                        encode_str(k, out);
                        out.push(':');
                        out.push(' ');
                        emit(value, out, level + 1, indent);
                    }
                    newline_at(out, level);
                    out.push('}');
                }
            }
        }

        pub fn to_value(v: &Pj) -> Value {
            match v {
                Pj::Null => Value::Null,
                Pj::Bool(b) => Value::Bool(*b),
                Pj::Int(s) => s
                    .parse::<i64>()
                    .map(|n| Value::from(n))
                    .unwrap_or_else(|_| Value::String(s.clone())),
                Pj::Float(f) => serde_json::Number::from_f64(*f)
                    .map(Value::Number)
                    .unwrap_or(Value::Null),
                Pj::Str(s) => Value::String(s.clone()),
                Pj::Arr(items) => Value::Array(items.iter().map(|x| to_value(x)).collect()),
                Pj::Obj(fields) => {
                    let mut map = Map::new();
                    for (k, value) in fields {
                        map.insert(k.clone(), to_value(value));
                    }
                    Value::Object(map)
                }
            }
        }
    }

    /// `json.dumps(data, ensure_ascii=False, indent=2)` — no trailing newline,
    /// which is exactly what `save_json` puts on disk.
    pub fn to_pretty(v: &Pj) -> String {
        let mut out = String::new();
        emit(v, &mut out, 0, Some(2));
        out
    }

    /// `json.dumps(data, ensure_ascii=False)` — the compact separators
    /// `_send_json` uses for responses.
    pub fn to_compact(v: &Pj) -> String {
        let mut out = String::new();
        emit(v, &mut out, 0, None);
        out
    }

    // ----------------------------------------------------------------- parse

    struct Scanner<'a> {
        c: Vec<char>,
        i: usize,
        _src: &'a str,
    }

    impl<'a> Scanner<'a> {
        fn peek(&self) -> Option<char> {
            self.c.get(self.i).copied()
        }
        fn skip_ws(&mut self) {
            while matches!(self.peek(), Some(' ') | Some('\t') | Some('\n') | Some('\r')) {
                self.i += 1;
            }
        }
        fn literal(&mut self, word: &str) -> Result<(), String> {
            for ch in word.chars() {
                if self.peek() != Some(ch) {
                    return Err(format!("expected {word}"));
                }
                self.i += 1;
            }
            Ok(())
        }
        fn scan_string(&mut self) -> Result<String, String> {
            self.i += 1; // opening quote
            let mut out = String::new();
            loop {
                let c = self.peek().ok_or_else(|| "unterminated string".to_string())?;
                self.i += 1;
                match c {
                    '"' => return Ok(out),
                    '\\' => {
                        let e = self.peek().ok_or_else(|| "unterminated escape".to_string())?;
                        self.i += 1;
                        match e {
                            '"' => out.push('"'),
                            '\\' => out.push('\\'),
                            '/' => out.push('/'),
                            'b' => out.push('\u{8}'),
                            'f' => out.push('\u{c}'),
                            'n' => out.push('\n'),
                            'r' => out.push('\r'),
                            't' => out.push('\t'),
                            'u' => {
                                let cp = self.hex4()?;
                                if (0xdc00..0xe000).contains(&cp) {
                                    return Err("unexpected low surrogate".to_string());
                                }
                                if (0xd800..0xdc00).contains(&cp) {
                                    // A lone high surrogate cannot be stored in
                                    // a Rust string; CPython keeps it and then
                                    // fails to encode it.  Map to U+FFFD.
                                    let combined = if self.peek() == Some('\\')
                                        && self.c.get(self.i + 1) == Some(&'u')
                                    {
                                        let save = self.i;
                                        self.i += 2;
                                        let low = self.hex4()?;
                                        if (0xdc00..0xe000).contains(&low) {
                                            Some(((cp - 0xd800) * 0x400) + (low - 0xdc00) + 0x10000)
                                        } else {
                                            self.i = save;
                                            None
                                        }
                                    } else {
                                        None
                                    };
                                    match combined {
                                        Some(n) => out.push(
                                            char::from_u32(n).unwrap_or(char::REPLACEMENT_CHARACTER),
                                        ),
                                        None => out.push(char::REPLACEMENT_CHARACTER),
                                    }
                                } else {
                                    out.push(
                                        char::from_u32(cp).unwrap_or(char::REPLACEMENT_CHARACTER),
                                    );
                                }
                            }
                            other => return Err(format!("invalid escape \\{other}")),
                        }
                    }
                    other if (other as u32) < 0x20 => {
                        return Err("invalid control character in string".to_string())
                    }
                    other => out.push(other),
                }
            }
        }
        fn hex4(&mut self) -> Result<u32, String> {
            let mut v: u32 = 0;
            for _ in 0..4 {
                let c = self.peek().ok_or_else(|| "truncated \\u escape".to_string())?;
                self.i += 1;
                let d = c.to_digit(16).ok_or_else(|| "invalid \\u escape".to_string())?;
                v = v * 16 + d;
            }
            Ok(v)
        }
        fn scan_number(&mut self) -> Result<Pj, String> {
            let start = self.i;
            if self.peek() == Some('-') {
                self.i += 1;
            }
            let mut is_float = false;
            match self.peek() {
                Some('0') => self.i += 1,
                Some(c) if c.is_ascii_digit() => {
                    while matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                        self.i += 1;
                    }
                }
                _ => return Err("invalid number".to_string()),
            }
            if self.peek() == Some('.') {
                is_float = true;
                self.i += 1;
                if !matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                    return Err("invalid fraction".to_string());
                }
                while matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                    self.i += 1;
                }
            }
            if matches!(self.peek(), Some('e') | Some('E')) {
                is_float = true;
                self.i += 1;
                if matches!(self.peek(), Some('+') | Some('-')) {
                    self.i += 1;
                }
                if !matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                    return Err("invalid exponent".to_string());
                }
                while matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                    self.i += 1;
                }
            }
            let text: String = self.c[start..self.i].iter().collect();
            if is_float {
                Ok(Pj::Float(text.parse::<f64>().map_err(|e| e.to_string())?))
            } else {
                Ok(Pj::Int(text))
            }
        }
        fn scan_value(&mut self, depth: usize) -> Result<Pj, String> {
            if depth > super::MAX_JSON_DEPTH {
                return Err("recursion limit exceeded".to_string());
            }
            self.skip_ws();
            match self.peek() {
                None => Err("expected value".to_string()),
                Some('{') => {
                    self.i += 1;
                    let mut fields: Vec<(String, Pj)> = Vec::new();
                    self.skip_ws();
                    if self.peek() == Some('}') {
                        self.i += 1;
                        return Ok(Pj::Obj(fields));
                    }
                    loop {
                        self.skip_ws();
                        if self.peek() != Some('"') {
                            return Err("expecting property name".to_string());
                        }
                        let key = self.scan_string()?;
                        self.skip_ws();
                        if self.peek() != Some(':') {
                            return Err("expecting colon".to_string());
                        }
                        self.i += 1;
                        let value = self.scan_value(depth + 1)?;
                        if let Some(slot) = fields.iter_mut().find(|(k, _)| *k == key) {
                            slot.1 = value; // duplicate keys: last one wins
                        } else {
                            fields.push((key, value));
                        }
                        self.skip_ws();
                        match self.peek() {
                            Some(',') => self.i += 1,
                            Some('}') => {
                                self.i += 1;
                                return Ok(Pj::Obj(fields));
                            }
                            _ => return Err("expecting , or }".to_string()),
                        }
                    }
                }
                Some('[') => {
                    self.i += 1;
                    let mut items: Vec<Pj> = Vec::new();
                    self.skip_ws();
                    if self.peek() == Some(']') {
                        self.i += 1;
                        return Ok(Pj::Arr(items));
                    }
                    loop {
                        let value = self.scan_value(depth + 1)?;
                        items.push(value);
                        self.skip_ws();
                        match self.peek() {
                            Some(',') => self.i += 1,
                            Some(']') => {
                                self.i += 1;
                                return Ok(Pj::Arr(items));
                            }
                            _ => return Err("expecting , or ]".to_string()),
                        }
                    }
                }
                Some('"') => Ok(Pj::Str(self.scan_string()?)),
                Some('t') => {
                    self.literal("true")?;
                    Ok(Pj::Bool(true))
                }
                Some('f') => {
                    self.literal("false")?;
                    Ok(Pj::Bool(false))
                }
                Some('n') => {
                    self.literal("null")?;
                    Ok(Pj::Null)
                }
                // `json.load` accepts these three by default (`parse_constant`).
                Some('N') => {
                    self.literal("NaN")?;
                    Ok(Pj::Float(f64::NAN))
                }
                Some('I') => {
                    self.literal("Infinity")?;
                    Ok(Pj::Float(f64::INFINITY))
                }
                Some('-') if self.c.get(self.i + 1) == Some(&'I') => {
                    self.literal("-Infinity")?;
                    Ok(Pj::Float(f64::NEG_INFINITY))
                }
                Some(c) if c == '-' || c.is_ascii_digit() => self.scan_number(),
                Some(c) => Err(format!("unexpected character {c}")),
            }
        }
    }

    /// `json.load` with the same strictness: any error means the caller falls
    /// back to its default, mirroring `utils.load_json`.
    pub fn parse(src: &str) -> Result<Pj, String> {
        fn scan(src: &str) -> Result<Pj, String> {
            let mut s = Scanner { c: src.chars().collect(), i: 0, _src: src };
            let v = s.scan_value(0)?;
            s.skip_ws();
            if s.i != s.c.len() {
                return Err("extra data".to_string());
            }
            Ok(v)
        }

        // `scan_value` recurses once per container level and a document at
        // `MAX_JSON_DEPTH` is legal, so the full budget is ~1000 live frames plus
        // the same depth of `Vec` drop glue when the tree is freed.  That does not
        // fit a default 2 MiB thread stack, and overflowing it is a crash where
        // CPython reports `RecursionError`.
        const SCAN_STACK_BYTES: usize = 32 << 20;
        std::thread::scope(|scope| {
            let spawned = std::thread::Builder::new()
                .stack_size(SCAN_STACK_BYTES)
                .spawn_scoped(scope, move || scan(src));
            match spawned {
                Ok(h) => match h.join() {
                    Ok(r) => r,
                    Err(_) => Err("parser panicked".to_string()),
                },
                Err(_) => scan(src),
            }
        })
    }
}

/// `utils.load_json`: unreadable, non-UTF-8 or malformed files yield `default`.
pub fn load_json(path: &Path, default: Pj) -> Pj {
    if !path.is_file() {
        return default;
    }
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return default,
    };
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(_) => return default,
    };
    match pyjson::parse(&text) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("读取 JSON 失败 {}: {}", path.display(), e);
            default
        }
    }
}

fn tmp_suffix() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{:08x}{:04x}", nanos, std::process::id() & 0xffff)
}

fn is_transient_rename_error(e: &std::io::Error) -> bool {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        return true;
    }
    match e.raw_os_error() {
        // 5 ERROR_ACCESS_DENIED, 32 sharing violation, 33 lock violation.
        Some(5) | Some(32) | Some(33) => true,
        _ => false,
    }
}

/// `utils.save_json`: unique sibling temp file, fsync, then `os.replace` with
/// six escalating retries.  Returns `false` on any failure, never panics.
pub fn save_json(path: &Path, data: &Pj) -> std::result::Result<(), String> {
    let target = path.to_path_buf();
    let dir = target
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let text = pyjson::to_pretty(data);
    (|| -> std::io::Result<()> {
        std::fs::create_dir_all(&dir)?;
        let base = target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "data.json".to_string());
        let tmp = dir.join(format!("{base}.{}.tmp", tmp_suffix()));
        {
            use std::io::Write;
            let mut f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
            f.write_all(text.as_bytes())?;
            f.flush()?;
            f.get_ref().sync_all()?;
        }
        let mut last: Option<std::io::Error> = None;
        for attempt in 0..6u32 {
            match std::fs::rename(&tmp, &target) {
                Ok(()) => {
                    if let Some(prev) = &last {
                        log::debug!("retried replace after {prev}");
                    }
                    return Ok(());
                }
                Err(e) if is_transient_rename_error(&e) && attempt < 5 => {
                    last = Some(e);
                    std::thread::sleep(std::time::Duration::from_millis(30 * (attempt as u64 + 1)));
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&tmp);
                    return Err(e);
                }
            }
        }
        let _ = std::fs::remove_file(&tmp);
        Err(last.unwrap_or_else(|| std::io::Error::other("replace failed")))
    })()
    .map_err(|e| {
        log::error!("保存 JSON 失败 {}: {e}", path.display());
        e.to_string()
    })
}

/// Python's `for x in rec` element kinds: list → items, dict → keys, str →
/// characters.
///
/// `Err` models the `TypeError` for anything else.  In `Api.add_recent`
/// (`readmd.py:5801`) and `Api.remove_recent` (`readmd.py:5815`) that
/// comprehension sits inside a `try`, and the `except Exception` fallback
/// repeats the same iteration — so a non-iterable `rec` raises `TypeError` a
/// **second time from inside the `except` block**, and that is what escapes the
/// method.  `_api_recent_add` maps a non-`ValueError` to
/// `500 {'code': 'recent_add_failed'}` (`readmd.py:2275`) and
/// `_api_recent_remove` to `recent_remove_failed` (`readmd.py:2297`).
fn iterate_like_python(v: &Pj) -> std::result::Result<Vec<Pj>, ()> {
    match v {
        Pj::Arr(items) => Ok(items.clone()),
        Pj::Obj(fields) => Ok(fields.iter().map(|(k, _)| Pj::Str(k.clone())).collect()),
        Pj::Str(s) => Ok(s.chars().map(|c| Pj::Str(c.to_string())).collect()),
        _ => Err(()),
    }
}

pub fn recent_path(data_dir: &Path) -> PathBuf {
    data_dir.join(RECENT_FILE)
}

pub fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SETTINGS_FILE)
}

/// `Api.get_recent` — `load_json(RECENT_FILE, [])`.
pub fn get_recent(data_dir: &Path) -> Pj {
    load_json(&recent_path(data_dir), Pj::empty_arr())
}

/// `Api.add_recent`: normcase-dedupe, insert at the front, keep 20.
pub fn add_recent(data_dir: &Path, path: &str) -> std::result::Result<(), String> {
    let raw = get_recent(data_dir);
    let rec = iterate_like_python(&raw).map_err(|_| "recent_not_iterable".to_string())?;
    let kept = (|| -> std::result::Result<Vec<Pj>, ()> {
        let target = crate::validators::normcase(path);
        let mut out = Vec::with_capacity(rec.len());
        for x in rec.iter() {
            let s = x.as_str().ok_or(())?;
            if crate::validators::normcase(s) != target {
                out.push(x.clone());
            }
        }
        Ok(out)
    })()
    .unwrap_or_else(|_| {
        rec.into_iter()
            .filter(|x| x != &Pj::Str(path.to_string()))
            .collect()
    });
    let mut kept = kept;
    kept.insert(0, Pj::Str(path.to_string()));
    kept.truncate(RECENT_KEEP);
    // Python ignores `save_json`'s `False` here and still returns `True`.
    let _ = save_json(&recent_path(data_dir), &Pj::Arr(kept));
    Ok(())
}

/// `Api.clear_recent` — writes a literal `[]`.
pub fn clear_recent(data_dir: &Path) -> std::result::Result<(), String> {
    // `save_json` logs its own failure; Python still reports success.
    let _ = save_json(&recent_path(data_dir), &Pj::empty_arr());
    Ok(())
}

/// `Api.remove_recent`: `''` is a no-op returning `false`; otherwise filter by
/// `normcase(normpath(...))` and rewrite the whole list.
pub fn remove_recent(data_dir: &Path, path: &str) -> std::result::Result<bool, String> {
    if path.is_empty() {
        return Ok(false);
    }
    let raw = get_recent(data_dir);
    let rec = iterate_like_python(&raw).map_err(|_| "recent_not_iterable".to_string())?;
    let kept = (|| -> std::result::Result<Vec<Pj>, ()> {
        let target = crate::validators::normcase(&crate::validators::normpath(path));
        let mut out = Vec::with_capacity(rec.len());
        for x in rec.iter() {
            let s = x.as_str().ok_or(())?;
            if crate::validators::normcase(&crate::validators::normpath(s)) != target {
                out.push(x.clone());
            }
        }
        Ok(out)
    })()
    .unwrap_or_else(|_| {
        rec.into_iter()
            .filter(|x| x != &Pj::Str(path.to_string()))
            .collect()
    });
    // Python ignores the write result here and still returns `True`.
    let _ = save_json(&recent_path(data_dir), &Pj::Arr(kept));
    Ok(true)
}

/// `Api.save_settings`: `cur = load_json(SETTINGS_FILE, {})`,
/// `cur.update(settings or {})`, `save_json(SETTINGS_FILE, cur)`, `return True`.
///
/// The merge is **shallow**, like `dict.update`: a top-level key present in
/// `patch` is replaced wholesale, and every untouched key keeps the exact value
/// read from disk — including one Python stored as `null`, which must not be
/// dropped or turned into `""`.
///
/// `Err` is reserved for the cases where Python's `cur.update` itself raises
/// (`TypeError` for a non-iterable, `ValueError` for a pair that does not
/// unpack) or where the on-disk file is not an object (`AttributeError` on
/// `cur.update`).  A deliberately narrowed subset: the exotic
/// `dict.update(["ab"])` → `{"a": "b"}` string-unpacking is not reproduced.
pub fn merge_settings(data_dir: &Path, patch: &Pj) -> std::result::Result<Pj, String> {
    let cur = load_json(&settings_path(data_dir), Pj::empty_obj());
    let mut fields = match cur {
        Pj::Obj(f) => f,
        _ => return Err("settings_not_object".to_string()),
    };
    let patch_fields: Vec<(String, Pj)> = match patch {
        Pj::Obj(f) => f.clone(),
        // `settings or {}` — every falsy patch updates nothing.
        other if !other.truthy() => Vec::new(),
        Pj::Arr(items) => {
            let mut out: Vec<(String, Pj)> = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Pj::Arr(pair) if pair.len() == 2 => {
                        let key = match &pair[0] {
                            Pj::Str(s) => s.clone(),
                            other => {
                                return Err(format!("settings_key_unsupported: {other:?}"))
                            }
                        };
                        out.push((key, pair[1].clone()));
                    }
                    other => return Err(format!("settings_pair_does_not_unpack: {other:?}")),
                }
            }
            out
        }
        // A `str` is iterable, so Python unpacks each single character as a
        // pair and raises `ValueError`; only reached when non-empty, because an
        // empty string is falsy and short-circuits above.
        Pj::Str(_) => {
            return Err("settings_pair_does_not_unpack: str".to_string());
        }
        other => return Err(format!("settings_not_mapping: {other:?}")),
    };
    for (k, v) in patch_fields {
        match fields.iter_mut().find(|(ek, _)| *ek == k) {
            Some(slot) => slot.1 = v,
            None => fields.push((k, v)),
        }
    }
    let merged = Pj::Obj(fields);
    // `Api.save_settings` ignores `save_json`'s return value and still
    // `return True`, so a failed write is not reported here either.
    let _ = save_json(&settings_path(data_dir), &merged);
    Ok(merged)
}

// --------------------------------------------------- recent file status probe

/// One entry of `Api.check_recent_status`'s `items` list, in the key order
/// Python emits: `path`, `status`, `resolved_path`, `name`, `dir`.
#[derive(Debug, Clone)]
pub struct RecentProbe {
    pub path: String,
    pub status: &'static str,
    pub resolved_path: String,
    pub name: String,
    pub dir: String,
}

impl RecentProbe {
    pub fn to_json(&self) -> Pj {
        Pj::Obj(vec![
            ("path".to_string(), Pj::Str(self.path.clone())),
            ("status".to_string(), Pj::Str(self.status.to_string())),
            ("resolved_path".to_string(), Pj::Str(self.resolved_path.clone())),
            ("name".to_string(), Pj::Str(self.name.clone())),
            ("dir".to_string(), Pj::Str(self.dir.clone())),
        ])
    }
}

/// `Api.check_recent_status` (`readmd.py:5828`).
///
/// The `Err` strings are Python **messages**, not wire codes.  Both
/// `raise ValueError('recent_paths_must_be_list')` (`readmd.py:5835`) and
/// `raise ValueError('invalid recent path')` (`readmd.py:5840`) are caught by
/// the same `except ValueError` in `_api_recent_status`, which answers
/// `400 {'ok': False, 'code': 'invalid_recent_paths'}` (`readmd.py:2258`) —
/// Python collapses them into **one** code, so a caller must not forward the
/// message verbatim.  Only a non-`ValueError` reaches
/// `500 {'code': 'recent_status_failed'}` (`readmd.py:2261`).
///
/// Python returns `{'ok': True, 'items': results}`; wrapping this `Vec` back
/// into that envelope is the caller's job.
pub fn check_recent_status(
    data_dir: &Path,
    paths: Option<&Pj>,
) -> std::result::Result<Vec<RecentProbe>, String> {
    let owned;
    let value = match paths {
        Some(v) => v,
        None => {
            owned = get_recent(data_dir);
            &owned
        }
    };
    // `if isinstance(paths, str): paths = [paths]` followed by the
    // `(list, tuple)` type gate: everything else raises `recent_paths_must_be_list`.
    let list: Vec<Pj> = match value {
        Pj::Arr(items) => items.clone(),
        Pj::Str(s) => vec![Pj::Str(s.clone())],
        _ => return Err("recent_paths_must_be_list".to_string()),
    };
    let mut results: Vec<RecentProbe> = Vec::new();
    for item in list.into_iter().take(MAX_RECENT_ENTRIES) {
        let p = match &item {
            Pj::Str(s) => s.clone(),
            _ => return Err("invalid recent path".to_string()),
        };
        if p.is_empty() || p.chars().count() > MAX_RECENT_PATH_LENGTH {
            return Err("invalid recent path".to_string());
        }
        results.push(probe_one(&p));
    }
    Ok(results)
}

fn probe_one(p: &str) -> RecentProbe {
    use crate::validators::{basename, dirname};
    let name = basename(p);
    let dir = dirname(p);
    if Path::new(p).is_file() {
        return RecentProbe {
            path: p.to_string(),
            status: "exists",
            resolved_path: p.to_string(),
            name,
            dir,
        };
    }
    // Bounded neighbour probe, mirroring `find_in` and its three stages.
    let mut moved: Option<String> = None;
    if !dir.is_empty() {
        moved = find_in(&dir, true, &name);
        if moved.is_none() {
            let parent = dirname(&dir);
            if !parent.is_empty() {
                moved = find_in(&parent, false, &name);
            }
        }
    }
    if moved.is_none() {
        let home = crate::validators::expanduser_home();
        for sub in ["Desktop", "Documents", "Downloads"] {
            // `os.path.join(os.path.join(home, sub), name)`: two joins, so the
            // separator before `name` is the platform one.
            let candidate =
                crate::validators::join_path(&crate::validators::join_path(&home, sub), &name);
            if Path::new(&candidate).is_file() {
                moved = Some(candidate);
                break;
            }
        }
    }
    match moved {
        Some(found) => RecentProbe {
            path: p.to_string(),
            status: "moved",
            resolved_path: found.clone(),
            name,
            dir: dirname(&found),
        },
        None => RecentProbe {
            path: p.to_string(),
            status: "deleted",
            resolved_path: p.to_string(),
            name,
            dir,
        },
    }
}

/// `find_in(directory, include_children)`: scan at most
/// `MAX_RECENT_SCAN_ENTRIES` entries, then optionally one level into at most
/// `MAX_RECENT_CHILD_DIRS` sub-directories.
fn find_in(directory: &str, include_children: bool, name: &str) -> Option<String> {
    let dir = Path::new(directory);
    if directory.is_empty() || !dir.is_dir() {
        return None;
    }
    let read = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return None,
    };
    let mut children: Vec<PathBuf> = Vec::new();
    for (checked, entry) in read.enumerate() {
        if checked + 1 > MAX_RECENT_SCAN_ENTRIES {
            break;
        }
        let entry = match entry {
            // `except OSError: return None` wraps the whole scandir loop.
            Ok(e) => e,
            Err(_) => return None,
        };
        if entry.file_name().to_string_lossy() == name {
            match entry.file_type() {
                Ok(t) if t.is_file() => {
                    return Some(entry.path().to_string_lossy().into_owned())
                }
                _ => {}
            }
        }
        if include_children {
            if let Ok(t) = entry.file_type() {
                if t.is_dir() {
                    children.push(entry.path());
                }
            }
        }
    }
    if !include_children {
        return None;
    }
    for child in children.into_iter().take(MAX_RECENT_CHILD_DIRS) {
        let read = match std::fs::read_dir(&child) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for entry in read.flatten() {
            if entry.file_name().to_string_lossy() == name {
                if let Ok(t) = entry.file_type() {
                    if t.is_file() {
                        return Some(entry.path().to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizer_handles_mixed_scripts() {
        let tokens = tokenize("ReadMD 文档索引 reader");
        let names: HashMap<String, i32> = tokens.into_iter().collect();
        assert!(names.contains_key("readmd"));
        assert!(names.contains_key("reader"));
        assert!(names.contains_key("文"));
        assert!(names.contains_key("文档"));
    }

    #[test]
    fn upsert_search_and_delete_roundtrip() {
        let store = Store::in_memory().unwrap();
        store
            .upsert_document("notes/a.md", "A", 10, 100, 12, "md", "# A\nreadmd kernel 文档索引")
            .unwrap();
        store
            .upsert_document("notes/b.md", "B", 20, 200, 8, "md", "# B\ncompletely unrelated text")
            .unwrap();
        let hits = store.search("readmd", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "notes/a.md");
        assert!(hits[0].snippet.contains("readmd"));
        let zh = store.search("文档", 10).unwrap();
        assert_eq!(zh.len(), 1);
        store.delete_document("notes/a.md").unwrap();
        assert!(store.search("readmd", 10).unwrap().is_empty());
        assert!(store.get_doc("notes/a.md").unwrap().is_none());
    }

    #[test]
    fn rename_moves_links_and_recents() {
        let store = Store::in_memory().unwrap();
        store.upsert_document("a.md", "A", 1, 1, 1, "md", "alpha").unwrap();
        store.upsert_document("b.md", "B", 1, 1, 1, "md", "beta").unwrap();
        store.replace_links("a.md", &["b.md".to_string()]).unwrap();
        store.touch_recent("a.md", "A").unwrap();
        store.rename_document("a.md", "c.md").unwrap();
        assert_eq!(store.backlinks("b.md").unwrap(), vec!["c.md".to_string()]);
        assert_eq!(store.recent(5).unwrap()[0].path, "c.md");
    }

    #[test]
    fn deadlinks_report_missing_targets() {
        let store = Store::in_memory().unwrap();
        store.upsert_document("a.md", "A", 1, 1, 1, "md", "alpha").unwrap();
        store
            .replace_links("a.md", &["missing.md".to_string()])
            .unwrap();
        let dead = store.deadlinks().unwrap();
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0]["target"], "missing.md");
    }

    #[test]
    fn chat_history_keeps_order() {
        let store = Store::in_memory().unwrap();
        store.remember_chat("s1", "user", "hi", "m").unwrap();
        store.remember_chat("s1", "assistant", "hello", "m").unwrap();
        let hist = store.chat_history("s1", 10).unwrap();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0]["role"], "user");
        store.clear_chat("s1").unwrap();
        assert!(store.chat_history("s1", 10).unwrap().is_empty());
    }

    #[test]
    fn stats_counts_indexed_corpus() {
        let store = Store::in_memory().unwrap();
        store.upsert_document("a.md", "A", 1, 1, 5, "md", "one two three").unwrap();
        let s = store.stats().unwrap();
        assert_eq!(s["docs"], 1);
        assert_eq!(s["words"], 5);
        assert!(s["tokens"].as_i64().unwrap_or(0) >= 3);
        let err: Error = std::io::Error::new(std::io::ErrorKind::NotFound, "not found").into();
        assert!(err.to_string().contains("not found"));
    }

    // ------------------------------------------------------- WD10: migration

    /// A fresh directory per case, cleaned of anything a previous run left.
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "readmd-wd10-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn open_backfills_columns_added_after_the_table_was_created() {
        let dir = temp_dir("migrate");
        let db = dir.join("readmd.db");
        // A `docs` table as an earlier kernel build would have left it: the two
        // key columns only.  `CREATE TABLE IF NOT EXISTS` alone could never add
        // the rest, and every write would then fail with `no such column`.
        {
            let conn = Connection::open(&db).unwrap();
            conn.execute_batch(
                "CREATE TABLE docs (id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE);
                 INSERT INTO docs (id, path) VALUES (7, 'legacy/a.md');",
            )
            .unwrap();
        }

        let store = Store::open(&db).unwrap();
        store
            .upsert_document("legacy/b.md", "B", 5, 6, 7, "md", "readmd kernel body")
            .unwrap();
        store.set_pinned("legacy/b.md", true).unwrap();

        // The pre-existing row now reads back with the schema defaults.
        let old = store.get_doc("legacy/a.md").unwrap().unwrap();
        assert_eq!(old.id, 7);
        assert_eq!(old.title, "");
        assert_eq!(old.mtime, 0);
        assert_eq!(old.size, 0);
        assert_eq!(old.words, 0);
        assert_eq!(old.kind, "md");
        assert!(!old.pinned);
        assert!(!store.search("kernel", 10).unwrap().is_empty());

        // Reopen from scratch: the backfilled columns are real and persisted.
        drop(store);
        let again = Store::open(&db).unwrap();
        let doc = again.get_doc("legacy/b.md").unwrap().unwrap();
        assert_eq!(doc.title, "B");
        assert_eq!(doc.mtime, 5);
        assert_eq!(doc.size, 6);
        assert_eq!(doc.words, 7);
        assert_eq!(doc.kind, "md");
        assert!(doc.pinned);
        assert_eq!(again.stats().unwrap()["docs"], 2);
    }

    #[test]
    fn open_reports_a_schema_it_cannot_repair() {
        let dir = temp_dir("migrate-tooold");
        let db = dir.join("readmd.db");
        {
            let conn = Connection::open(&db).unwrap();
            // `path` is part of the canonical unique key, so SQLite forbids
            // adding it in place: this is not a repairable legacy file.
            conn.execute("CREATE TABLE docs (id INTEGER PRIMARY KEY)", [])
                .unwrap();
        }
        let err = match Store::open(&db) {
            Ok(_) => panic!("a schema this build cannot write to must be refused at open"),
            Err(e) => e,
        };
        let text = err.to_string();
        assert!(
            text.contains("db_schema_too_old") && text.contains("docs.path"),
            "unexpected error: {text}"
        );
    }

    #[test]
    fn open_refuses_a_schema_from_the_future() {
        let dir = temp_dir("migrate-future");
        let db = dir.join("readmd.db");
        Store::open(&db).unwrap();
        {
            let conn = Connection::open(&db).unwrap();
            conn.execute(
                "INSERT INTO meta (key, value) VALUES ('schema_version', '2') \
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [],
            )
            .unwrap();
        }
        let err = match Store::open(&db) {
            Ok(_) => panic!("a newer schema must not be adopted"),
            Err(e) => e.to_string(),
        };
        assert!(
            err.contains("db_schema_too_new"),
            "a newer schema must not be relabelled: {err}"
        );
        // And the file was not stamped back down to this kernel's version.
        let conn = Connection::open(&db).unwrap();
        let stored: String = conn
            .query_row("SELECT value FROM meta WHERE key='schema_version'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(stored, "2");
    }

    #[test]
    fn open_creates_a_missing_database_and_parent_dirs() {
        let dir = temp_dir("missing-db");
        let db = dir.join("index").join("nested").join("readmd.db");
        assert!(!db.exists());
        let store = Store::open(&db).unwrap();
        assert!(db.is_file());
        let s = store.stats().unwrap();
        assert_eq!(s["docs"], 0);
        assert_eq!(s["tokens"], 0);
        assert_eq!(s["links"], 0);
        assert_eq!(s["words"], 0);
        // An empty file on disk is not a database either way; SQLite adopts it.
        drop(store);
        let again = Store::open(&db).unwrap();
        assert_eq!(again.stats().unwrap()["docs"], 0);
    }

    #[test]
    fn rename_onto_an_occupied_path_dedupes_instead_of_failing() {
        let store = Store::in_memory().unwrap();
        store.upsert_document("old.md", "Old", 10, 1, 1, "md", "keepme alpha").unwrap();
        // A stale entry for the path the user is renaming *onto*: the file that
        // lived there is gone but the index never learned about it.
        store.upsert_document("new.md", "Ghost", 1, 1, 1, "md", "ghostbody").unwrap();
        store.replace_links("old.md", &["t.md".into()]).unwrap();
        // This (new.md -> t.md) link makes the naive `UPDATE links` collide with
        // the primary key, exactly like `docs.path`'s UNIQUE did.
        store.replace_links("new.md", &["t.md".into()]).unwrap();
        // Both keys exist in `recent`, so the rename must dedupe rather than
        // abort on the primary key.
        store.touch_recent("old.md", "Old").unwrap();
        store.touch_recent("new.md", "Ghost").unwrap();

        store.rename_document("old.md", "new.md").unwrap();

        let doc = store.get_doc("new.md").unwrap().expect("renamed row");
        assert_eq!(doc.title, "Old", "the renamed file must win over the ghost");
        assert_eq!(store.stats().unwrap()["docs"], 1);
        assert!(store.get_doc("old.md").unwrap().is_none());
        // Renamed document still finds its own body; the ghost's tokens are gone.
        let hits = store.search("keepme", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "new.md");
        assert!(store.search("ghostbody", 10).unwrap().is_empty());
        // Links deduped to a single edge instead of aborting the rename.
        assert_eq!(store.backlinks("t.md").unwrap(), vec!["new.md".to_string()]);
        assert_eq!(store.stats().unwrap()["links"], 1);
        // `recent` also had a row at both keys.
        let paths: Vec<String> = store.recent(10).unwrap().into_iter().map(|r| r.path).collect();
        assert_eq!(paths, vec!["new.md".to_string()]);
    }

    // ---------------------------------------------------------- WD10: search

    #[test]
    fn search_keeps_a_deterministic_token_prefix_and_total_ordering() {
        let store = Store::in_memory().unwrap();
        for i in 0..20 {
            store
                .upsert_document(
                    &format!("d{i}.md"),
                    &format!("T{i}"),
                    100 - i as i64,
                    1,
                    1,
                    "md",
                    &format!("tok{i:02} sharedword"),
                )
                .unwrap();
        }
        let query = (0..20)
            .map(|i| format!("tok{i:02}"))
            .collect::<Vec<_>>()
            .join(" ");

        let first: Vec<String> = store
            .search(&query, 50)
            .unwrap()
            .into_iter()
            .map(|h| h.path)
            .collect();
        // The cap is still 12 tokens, but which 12 is now a rule, not a
        // `HashMap` roll of the dice.
        assert_eq!(first.len(), 12, "got {first:?}");
        for _ in 0..8 {
            let again: Vec<String> = store
                .search(&query, 50)
                .unwrap()
                .into_iter()
                .map(|h| h.path)
                .collect();
            assert_eq!(again, first, "search results must be stable across calls");
        }
    }

    #[test]
    fn search_breaks_score_ties_on_mtime_then_path() {
        let store = Store::in_memory().unwrap();
        // Identical body, identical mtime ⇒ identical score.
        store.upsert_document("z.md", "Z", 9, 1, 1, "md", "alphatoken").unwrap();
        store.upsert_document("a.md", "A", 9, 1, 1, "md", "alphatoken").unwrap();
        let hits = store.search("alphatoken", 10).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].path, "a.md");
        assert_eq!(hits[1].path, "z.md");
        assert_eq!(hits[0].score, hits[1].score);

        // Same score, differing mtime ⇒ the newer document wins regardless of path.
        store.upsert_document("m.md", "M", 50, 1, 1, "md", "alphatoken").unwrap();
        let hits = store.search("alphatoken", 10).unwrap();
        assert_eq!(hits[0].path, "m.md");
    }

    #[test]
    fn snippet_anchor_survives_length_expanding_lowercase() {
        // `İ` (U+0130) full-lowercases to `i` + U+0307, so 40 of them make the
        // match text twice as long as the source.  The old code sliced `chars`
        // with an index counted in the *lowered* text, which put `start` past
        // `chars.len()` and panicked ("range ends before begin") as soon as the
        // matched token sat behind such a run.
        let store = Store::in_memory().unwrap();
        let body = format!("{} needleend", "\u{130}".repeat(40));
        store
            .upsert_document("tr.md", "TR", 5, 1, 1, "md", &body)
            .unwrap();
        // Empty body must stay safe too (no `origin` entries at all).
        store.upsert_document("empty.md", "E", 5, 1, 1, "md", "").unwrap();

        let hits = store.search("needleend", 10).unwrap();
        assert_eq!(hits.len(), 1, "got {hits:?}");
        let sn = &hits[0].snippet;
        assert!(
            sn.chars().count() <= SNIPPET_RADIUS * 2 + 6,
            "snippet runaway: {sn:?}"
        );
        assert!(sn.contains("needleend"), "snippet lost the match: {sn:?}");

        // A doc indexed with an empty body is still returned when it matches on
        // nothing; the snippet just has to be a plain string, not a panic.
        let hits = store.search("needleend", 10).unwrap();
        assert_eq!(hits.len(), 1);
    }

    // ---------------------------------------------------------- WD10: pyjson

    #[test]
    fn parser_matches_cpythons_depth_limit_at_the_boundary() {
        let at_limit = format!("{}0{}", "[".repeat(1000), "]".repeat(1000));
        assert!(
            pyjson::parse(&at_limit).is_ok(),
            "CPython's json.load reads 1000 nested containers"
        );
        let over = format!("{}0{}", "[".repeat(1001), "]".repeat(1001));
        assert_eq!(
            pyjson::parse(&over).unwrap_err(),
            "recursion limit exceeded"
        );
        // A file Python loads must never be silently replaced by its default.
        let dir = temp_dir("depth");
        let ok = dir.join("ok.json");
        std::fs::write(&ok, &at_limit).unwrap();
        assert!(matches!(load_json(&ok, Pj::empty_arr()), Pj::Arr(_)));
        let bad = dir.join("bad.json");
        std::fs::write(&bad, &over).unwrap();
        assert_eq!(load_json(&bad, Pj::empty_arr()), Pj::empty_arr());
    }

    // ------------------------------------------------- write-path recursion

    /// A `Pj` nested `depth` containers deep, built iteratively.
    fn nest(depth: usize) -> Pj {
        let mut v = Pj::Int("0".into());
        for _ in 0..depth {
            v = Pj::Arr(vec![v]);
        }
        v
    }

    /// `json.dumps(nest(MAX_JSON_DEPTH), ensure_ascii=False, indent=2)` shape.
    fn pretty_nested_arrays(depth: usize) -> String {
        let mut out = String::new();
        for i in 0..depth {
            out.push_str(&" ".repeat(i * 2));
            out.push('[');
            out.push('\n');
        }
        out.push_str(&" ".repeat(depth * 2));
        out.push('0');
        for i in (0..depth).rev() {
            out.push('\n');
            out.push_str(&" ".repeat(i * 2));
            out.push(']');
        }
        out
    }

    #[test]
    fn write_path_uses_no_call_stack_at_the_parse_depth_ceiling() {
        // The parse gate deliberately accepts 1000 levels (see MAX_JSON_DEPTH), so
        // every write-path consumer has to survive them on a stack far smaller
        // than the 1 MiB main-thread reserve the shipped binary gets.  A 64 KiB
        // borrow-only thread is the assertion: the recursive emitter needed
        // ~400-650 KiB here before the rewrite and overflows this immediately.
        let deep = nest(MAX_JSON_DEPTH);
        let compact_src = format!("{}0{}", "[".repeat(MAX_JSON_DEPTH), "]".repeat(MAX_JSON_DEPTH));
        let pretty_src = pretty_nested_arrays(MAX_JSON_DEPTH);
        let value_probe = Pj::Obj(vec![("k".into(), deep.clone())]);
        let (compact, pretty, value) = std::thread::scope(|scope| {
            let handle = std::thread::Builder::new()
                .stack_size(64 << 10)
                .spawn_scoped(scope, || {
                    // Borrow only, and hand the `Value` back out of the thread:
                    // freeing a 1000-deep tree is still recursive (`Pj` and
                    // `Value` both use derived drop glue), and so is
                    // `serde_json`'s own serializer — neither is this fix's
                    // subject, so neither may run on this stack.
                    let c = pyjson::to_compact(&deep);
                    let p = pyjson::to_pretty(&deep);
                    let v = value_probe.to_value();
                    (c, p, v)
                })
                .expect("64 KiB thread must spawn")
                .join()
                .expect("write path must not panic or overflow");
            (handle.0, handle.1, handle.2)
        });
        assert_eq!(compact, compact_src);
        assert_eq!(pretty, pretty_src);
        // `{"k": [ ...1000 arrays... 0 ... ]}` — the bridge kept the nesting.
        let mut expected_value = String::from("{\"k\":");
        expected_value.push_str(&"[".repeat(MAX_JSON_DEPTH));
        expected_value.push('0');
        expected_value.push_str(&"]".repeat(MAX_JSON_DEPTH));
        expected_value.push('}');
        assert_eq!(value.to_string(), expected_value);
        assert!(value.get("k").is_some());
    }

    #[test]
    fn iterative_write_path_matches_the_recursive_one_it_replaced() {
        // Differential oracle: same bytes, same `Value`, over mixed shapes —
        // empty containers, multi-element siblings (the separator and ordering
        // cases), control characters, non-ASCII, big ints and non-finite floats.
        let mut seed: u64 = 0x243f_6a88_85a3_08d3;
        let mut next = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as usize
        };
        fn build(next: &mut dyn FnMut() -> usize, depth: usize) -> Pj {
            let leafish = depth == 0;
            let kinds = if leafish { 6 } else { 8 };
            match next() % kinds {
                0 => Pj::Null,
                1 => Pj::Bool(next() % 2 == 0),
                2 => Pj::Int(format!("{}{}", if next() % 3 == 0 { "-" } else { "" }, next())),
                3 => Pj::Float(match next() % 4 {
                    0 => f64::NAN,
                    1 => f64::INFINITY,
                    2 => -(next() as f64) / 7.0,
                    _ => 1e18,
                }),
                4 => Pj::Str(format!("a\"\u{0}\u{7f}\\\t{}é文", next() % 5)),
                5 => Pj::empty_arr(),
                6 => Pj::empty_obj(),
                _ => {
                    let n = next() % 4;
                    if next() % 2 == 0 {
                        Pj::Arr((0..n).map(|_| build(next, depth - 1)).collect())
                    } else {
                        Pj::Obj(
                            (0..n)
                                .map(|i| {
                                    (
                                        format!("k{}{}", i, if i % 2 == 0 { "b" } else { "a" }),
                                        build(next, depth - 1),
                                    )
                                })
                                .collect(),
                        )
                    }
                }
            }
        }
        for _ in 0..400 {
            let depth = 1 + next() % 9;
            let tree = build(&mut next, depth);
            let mut recursive_compact = String::new();
            pyjson::reference::emit(&tree, &mut recursive_compact, 0, None);
            assert_eq!(pyjson::to_compact(&tree), recursive_compact);
            let mut recursive_pretty = String::new();
            pyjson::reference::emit(&tree, &mut recursive_pretty, 0, Some(2));
            assert_eq!(pyjson::to_pretty(&tree), recursive_pretty);
            assert_eq!(tree.to_value(), pyjson::reference::to_value(&tree));
            assert_eq!(pyjson::reference::to_value(&tree), tree.to_value());
        }
    }

    #[test]
    fn deep_document_round_trips_and_over_limit_reports_the_parse_gate_error() {
        let dir = temp_dir("depth_write");
        let file = dir.join("deep.json");
        let deep = nest(MAX_JSON_DEPTH);
        let expected = pretty_nested_arrays(MAX_JSON_DEPTH);
        // The accepted depth survives `save_json` -> disk -> `load_json`.
        assert_eq!(save_json(&file, &deep), Ok(()));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), expected);
        assert_eq!(load_json(&file, Pj::empty_arr()), deep);
        // Nothing on the write side rejects it either: no cap was added.
        assert_eq!(pyjson::to_compact(&deep), format!("{}0{}", "[".repeat(1000), "]".repeat(1000)));

        // Over the gate, the *read* side keeps reporting the same error shape and
        // `load_json` keeps falling back to its default.
        let over_src = format!("{}0{}", "[".repeat(1001), "]".repeat(1001));
        assert_eq!(pyjson::parse(&over_src).unwrap_err(), "recursion limit exceeded");
        std::fs::write(&file, &over_src).unwrap();
        assert_eq!(load_json(&file, Pj::empty_arr()), Pj::empty_arr());

        // And no write-path entry point can panic, whatever depth it is handed:
        // a tree deeper than Python's own `dumps` ceiling is still serialised
        // rather than rejected, because rejecting it would lose user bytes.
        let absurd = nest(4000);
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_eq!(
                pyjson::to_compact(&absurd),
                format!("{}0{}", "[".repeat(4000), "]".repeat(4000))
            );
            assert!(pyjson::to_pretty(&absurd).ends_with(']'));
            assert_eq!(save_json(&file, &absurd), Ok(()));
        }))
        .is_err();
        assert!(!panicked, "the write path must never panic on deep input");
        // Deliberate leak: freeing a 4000-level tree is exactly the recursive
        // `Drop` exposure this lane documents rather than fixes (derived drop
        // glue cannot be made iterative here without changing `Pj` for callers
        // that destructure it by value).
        std::mem::forget(absurd);
        assert_eq!(load_json(&file, Pj::empty_arr()), Pj::empty_arr());
    }

    #[test]
    fn emitter_reproduces_cpythons_dump_shape_byte_for_byte() {
        // Every case below was captured from
        // `json.dumps(value, ensure_ascii=False, indent=2)` on CPython 3.11.
        let v = Pj::Obj(vec![
            ("n".into(), Pj::Float(1e20)),
            ("big".into(), Pj::Int("12345678901234567890".into())),
            ("f".into(), Pj::Float(-0.0)),
            ("e".into(), Pj::Float(1e16)),
            ("tiny".into(), Pj::Float(1.5e-5)),
            ("del".into(), Pj::Str("a\u{7f}b".into())),
            ("ctl".into(), Pj::Str("\u{0}\u{1f}".into())),
            ("tab".into(), Pj::Str("a\tb\nc\u{8}d\u{c}e\r".into())),
            ("zh".into(), Pj::Str("中文".into())),
            (
                "arr".into(),
                Pj::Arr(vec![
                    Pj::Int("1".into()),
                    Pj::Float(2.5),
                    Pj::Str("x".into()),
                    Pj::empty_arr(),
                    Pj::empty_obj(),
                ]),
            ),
            (
                // Insertion order, not `BTreeMap` order: `b` before `a`.
                "obj".into(),
                Pj::Obj(vec![
                    ("b".into(), Pj::Int("1".into())),
                    ("a".into(), Pj::Int("2".into())),
                ]),
            ),
            ("z".into(), Pj::Null),
        ]);
        let expected = [
            "{",
            "  \"n\": 1e+20,",
            "  \"big\": 12345678901234567890,",
            "  \"f\": -0.0,",
            "  \"e\": 1e+16,",
            "  \"tiny\": 1.5e-05,",
            "  \"del\": \"a\u{7f}b\",",
            "  \"ctl\": \"\\u0000\\u001f\",",
            "  \"tab\": \"a\\tb\\nc\\bd\\fe\\r\",",
            "  \"zh\": \"中文\",",
            "  \"arr\": [",
            "    1,",
            "    2.5,",
            "    \"x\",",
            "    [],",
            "    {}",
            "  ],",
            "  \"obj\": {",
            "    \"b\": 1,",
            "    \"a\": 2",
            "  },",
            "  \"z\": null",
            "}",
        ]
        .join("\n");
        assert_eq!(pyjson::to_pretty(&v), expected);

        // Compact form keeps `json.dumps`' default `(', ', ': ')` separators.
        let compact = pyjson::to_compact(&v);
        assert!(
            compact.starts_with("{\"n\": 1e+20, \"big\": 12345678901234567890, "),
            "{compact}"
        );
        assert!(compact.contains("\"arr\": [1, 2.5, \"x\", [], {}]"), "{compact}");
        assert!(compact.contains("\"obj\": {\"b\": 1, \"a\": 2}"), "{compact}");

        // And the parser reads Python's own bytes back without reordering.
        let reparsed = pyjson::parse(&expected).unwrap();
        assert_eq!(reparsed, v);
        assert_eq!(pyjson::to_pretty(&reparsed), expected);
    }

    // -------------------------------------------------------- WD10: settings

    #[test]
    fn settings_merge_is_shallow_and_keeps_untouched_keys_and_nulls() {
        let dir = temp_dir("settings");
        let file = settings_path(&dir);
        assert!(!file.exists());

        merge_settings(
            &dir,
            &Pj::Obj(vec![
                ("theme".into(), Pj::Str("dark".into())),
                ("字体".into(), Pj::Str("中文".into())),
                (
                    "style".into(),
                    Pj::Obj(vec![("css".into(), Pj::Str("a{color:red}".into()))]),
                ),
                // Python's `Api.save_settings({'pet_slug': None})` stores a
                // JSON null; the UI reads that differently from an absent key.
                ("pet_slug".into(), Pj::Null),
            ]),
        )
        .unwrap();

        // A partial update must not disturb anything it did not name.
        merge_settings(
            &dir,
            &Pj::Obj(vec![("fontSize".into(), Pj::Int("16".into()))]),
        )
        .unwrap();

        let back = load_json(&file, Pj::empty_obj());
        let fields = match &back {
            Pj::Obj(f) => f,
            other => panic!("settings.json must stay an object, got {other:?}"),
        };
        assert_eq!(
            fields.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
            vec!["theme", "字体", "style", "pet_slug", "fontSize"],
            "insertion order is what Python writes"
        );
        assert_eq!(fields[0].1, Pj::Str("dark".into()));
        assert_eq!(fields[1].1, Pj::Str("中文".into()));
        assert_eq!(
            back.get("style").and_then(|s| s.get("css")),
            Some(&Pj::Str("a{color:red}".into()))
        );
        assert_eq!(back.get("pet_slug"), Some(&Pj::Null));
        assert_eq!(back.get("fontSize"), Some(&Pj::Int("16".into())));

        // On-disk bytes: `ensure_ascii=False`, indent 2, no trailing newline.
        let raw = std::fs::read_to_string(&file).unwrap();
        assert_eq!(raw, pyjson::to_pretty(&back));
        assert!(
            raw.starts_with("{\n  \"theme\": \"dark\",\n  \"字体\": \"中文\","),
            "{raw}"
        );
        assert!(!raw.contains("\\u4e2d"), "non-ASCII must stay raw: {raw}");
        assert!(raw.contains("\"pet_slug\": null"), "null, not \"\": {raw}");
        assert!(!raw.ends_with('\n'));

        // A falsy patch updates nothing, like `settings or {}`.
        for falsy in [Pj::Null, Pj::Bool(false), Pj::Int("0".into()), Pj::Str(String::new())] {
            let merged = merge_settings(&dir, &falsy).unwrap();
            assert_eq!(merged, back, "{falsy:?} must not change the merge");
        }
        assert_eq!(load_json(&file, Pj::empty_obj()), back);
    }

    #[test]
    fn settings_merge_rejects_non_objects_like_python_update_does() {
        let dir = temp_dir("settings-bad");
        // `dict.update([["k", v]])` really does merge, so an array body must not
        // silently no-op the way it used to.
        let merged = merge_settings(
            &dir,
            &Pj::Arr(vec![Pj::Arr(vec![
                Pj::Str("x".into()),
                Pj::Int("1".into()),
            ])]),
        )
        .unwrap();
        assert_eq!(merged.get("x"), Some(&Pj::Int("1".into())));

        assert!(merge_settings(&dir, &Pj::Arr(vec![Pj::Str("nope".into())]))
            .unwrap_err()
            .starts_with("settings_pair_does_not_unpack"));
        assert!(merge_settings(&dir, &Pj::Str("ab".into()))
            .unwrap_err()
            .starts_with("settings_pair_does_not_unpack"));
        assert!(merge_settings(&dir, &Pj::Int("5".into()))
            .unwrap_err()
            .starts_with("settings_not_mapping"));

        // A settings file that is not an object: Python's `cur.update` raises
        // `AttributeError`, and the file itself must survive untouched.
        std::fs::write(settings_path(&dir), "[1, 2]").unwrap();
        assert_eq!(
            merge_settings(&dir, &Pj::Obj(vec![("a".into(), Pj::Int("1".into()))])).unwrap_err(),
            "settings_not_object"
        );
        assert_eq!(std::fs::read_to_string(settings_path(&dir)).unwrap(), "[1, 2]");
    }

    // ----------------------------------------------------------- WD10: recent

    #[test]
    fn recent_round_trip_truncates_to_twenty_and_dedupes() {
        let dir = temp_dir("recent");
        let file = recent_path(&dir);
        assert_eq!(get_recent(&dir), Pj::empty_arr());

        for i in 0..25 {
            add_recent(&dir, &format!("note {i:02}.md")).unwrap();
        }
        let items = get_recent(&dir);
        let arr = items.as_array().expect("recent.json is a list");
        assert_eq!(arr.len(), RECENT_KEEP, "`rec[:20]`");
        assert_eq!(arr[0].as_str(), Some("note 24.md"), "newest first");
        assert_eq!(arr[RECENT_KEEP - 1].as_str(), Some("note 05.md"));

        // Re-adding an existing entry moves it to the front without growing.
        add_recent(&dir, "note 12.md").unwrap();
        let arr = get_recent(&dir);
        let arr = arr.as_array().unwrap();
        assert_eq!(arr.len(), RECENT_KEEP);
        assert_eq!(arr[0].as_str(), Some("note 12.md"));

        let raw = std::fs::read_to_string(&file).unwrap();
        assert_eq!(raw, pyjson::to_pretty(&Pj::Arr(arr.to_vec())));
        assert_eq!(raw.matches("note ").count(), RECENT_KEEP);

        // `remove_recent('')` is a no-op that answers false.
        assert_eq!(remove_recent(&dir, "").unwrap(), false);
        assert_eq!(get_recent(&dir).as_array().unwrap().len(), RECENT_KEEP);
        assert_eq!(remove_recent(&dir, "note 12.md").unwrap(), true);
        let after = get_recent(&dir);
        let after = after.as_array().unwrap();
        assert_eq!(after.len(), RECENT_KEEP - 1);
        assert!(after.iter().all(|x| x.as_str() != Some("note 12.md")));
        // Python rewrites the whole list with no truncation here.
        assert_eq!(after[0].as_str(), Some("note 24.md"));

        clear_recent(&dir).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "[]");

        // A recent.json holding a non-list still errors the way Python's
        // `for x in rec` does.
        std::fs::write(&file, "5").unwrap();
        assert_eq!(
            add_recent(&dir, "x.md").unwrap_err(),
            "recent_not_iterable"
        );
        assert_eq!(
            remove_recent(&dir, "x.md").unwrap_err(),
            "recent_not_iterable"
        );
        std::fs::write(&file, r#"{"a": 1, "b": 2}"#).unwrap();
        assert_eq!(
            check_recent_status(&dir, None).unwrap_err(),
            "recent_paths_must_be_list"
        );
    }

    #[test]
    fn check_recent_status_bounds_and_reports_each_entry_kind() {
        let dir = temp_dir("recent-status");
        let present = dir.join("here.md");
        std::fs::write(&present, "# here\n").unwrap();

        let probes = check_recent_status(
            &dir,
            Some(&Pj::Arr(vec![
                Pj::Str(present.to_string_lossy().into_owned()),
                Pj::Str(dir.join("gone.md").to_string_lossy().into_owned()),
            ])),
        )
        .unwrap();
        assert_eq!(probes.len(), 2);
        assert_eq!(probes[0].status, "exists");
        assert_eq!(probes[0].resolved_path, probes[0].path);
        assert_eq!(probes[0].name, "here.md");
        assert_eq!(probes[0].dir, dir.to_string_lossy().into_owned());
        assert_eq!(probes[1].status, "deleted");
        assert_eq!(probes[1].resolved_path, probes[1].path);

        // Python's key order, which is what `save_json` writes to disk. These are
        // Windows paths, and CPython's `json.dumps` doubles every backslash — one
        // input `\` is emitted as the two-character escape `\\` — so the raw path
        // must never appear verbatim inside the JSON text. The expectation is
        // spelled out with `str::replace` instead of the emitter itself, so the
        // check stays honest.
        let present_s = present.to_string_lossy().into_owned();
        let dir_s = dir.to_string_lossy().into_owned();
        let bslash = char::from_u32(0x5c).unwrap().to_string();
        let py_escape = |s: &str| s.replace(bslash.as_str(), &format!("{bslash}{bslash}"));
        let emitted = pyjson::to_compact(&probes[0].to_json());
        assert_eq!(
            emitted,
            format!(
                "{{\"path\": \"{}\", \"status\": \"exists\", \"resolved_path\": \"{}\", \
                 \"name\": \"here.md\", \"dir\": \"{}\"}}",
                py_escape(&present_s),
                py_escape(&present_s),
                py_escape(&dir_s)
            )
        );

        // The bytes Python would read back are the paths it was handed: the
        // doubling is escaping, not data corruption.
        let back = pyjson::parse(&emitted).unwrap();
        let fields = match &back {
            Pj::Obj(fields) => fields,
            other => panic!("status entry must be an object, got {other:?}"),
        };
        assert_eq!(
            fields.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
            vec!["path", "status", "resolved_path", "name", "dir"],
            "insertion order is what Python writes"
        );
        assert_eq!(back.get("path").and_then(|s| s.as_str()), Some(present_s.as_str()));
        assert_eq!(
            back.get("resolved_path").and_then(|s| s.as_str()),
            Some(present_s.as_str())
        );
        assert_eq!(back.get("dir").and_then(|s| s.as_str()), Some(dir_s.as_str()));
        assert_eq!(back.get("name").and_then(|s| s.as_str()), Some("here.md"));
        assert_eq!(back.get("status").and_then(|s| s.as_str()), Some("exists"));

        // A bare string is wrapped into a one-element list, not iterated.
        assert_eq!(
            check_recent_status(&dir, Some(&Pj::Str("x.md".into())))
                .unwrap()
                .len(),
            1
        );
        // `paths[:MAX_RECENT_ENTRIES]` — 24, not `RECENT_KEEP`'s 20.
        let many = Pj::Arr(
            (0..MAX_RECENT_ENTRIES + 10)
                .map(|i| Pj::Str(format!("f{i}.md")))
                .collect(),
        );
        assert_eq!(check_recent_status(&dir, Some(&many)).unwrap().len(), 24);
        // Empty and over-long paths raise `ValueError('invalid recent path')`.
        assert_eq!(
            check_recent_status(&dir, Some(&Pj::Arr(vec![Pj::Str(String::new())])))
                .unwrap_err(),
            "invalid recent path"
        );
        let too_long = "x".repeat(MAX_RECENT_PATH_LENGTH + 1);
        assert_eq!(
            check_recent_status(&dir, Some(&Pj::Arr(vec![Pj::Str(too_long)]))).unwrap_err(),
            "invalid recent path"
        );
        assert_eq!(
            check_recent_status(&dir, Some(&Pj::Arr(vec![Pj::Int("1".into())]))).unwrap_err(),
            "invalid recent path"
        );
    }

    // --------------------------------------------------------- WD10: load_json

    #[test]
    fn load_json_falls_back_only_where_python_raises() {
        let dir = temp_dir("load-json");
        let default = Pj::empty_arr();
        let cases: [(&str, &[u8], bool); 7] = [
            ("empty", b"", true),
            ("bom", "\u{feff}{}".as_bytes(), true),
            ("truncated", b"{\"a\": ", true),
            ("unclosed", b"[1,2", true),
            ("bad-escape", b"[\"a\\qb\"]", true),
            ("bare-null", b"null", false),
            ("bare-int", b"5", false),
        ];
        for (name, bytes, expect_default) in cases {
            let path = dir.join(format!("{name}.json"));
            std::fs::write(&path, bytes).unwrap();
            let got = load_json(&path, default.clone());
            if expect_default {
                assert_eq!(got, default, "{name} must fall back like `except Exception`");
            } else {
                assert_ne!(got, default, "{name} is valid JSON; Python keeps the value");
            }
        }
        // Missing file and a directory both short-circuit on `isfile`.
        assert_eq!(load_json(&dir.join("absent.json"), default.clone()), default);
        assert_eq!(load_json(&dir, default.clone()), default);
        // Non-UTF-8 bytes: `UnicodeDecodeError` → default.
        let binary = dir.join("binary.json");
        std::fs::write(&binary, [0xff_u8, 0xfe, 0xfd]).unwrap();
        assert_eq!(load_json(&binary, default.clone()), default);
        // Duplicate object keys: last value wins, first position kept.
        let dupes = dir.join("dupes.json");
        std::fs::write(&dupes, b"{\"a\": 1, \"b\": 2, \"a\": 3}").unwrap();
        let got = load_json(&dupes, default.clone());
        assert_eq!(
            pyjson::to_compact(&got),
            "{\"a\": 3, \"b\": 2}",
            "matches CPython's dict semantics"
        );
    }
}
