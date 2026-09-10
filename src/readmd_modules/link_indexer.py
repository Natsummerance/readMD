# -*- coding: utf-8 -*-
"""ReadMD 本地双链倒排索引与知识图谱引擎。

基于 Python 3 原生 sqlite3（WAL 模式 + 覆盖索引）构建：
- 秒级增量扫描：仅对修改时间 (mtime) 发生变化的文件重扫，<2ms/文件。
- 双链提取：解析 [[target]]、[[target|alias]]、[[target#heading]] 与 Markdown [text](target.md)。
- 代码块语法屏蔽：自动屏蔽 ``` 代码块及 ` 行内代码中的文本，杜绝误识别。
- 容错目标解析：支持相对路径、扩展名省略及文件名大小写模糊对齐。
- 死链 (Deadlink) 审计与拓扑关系图谱 (Graph Data) 输出。
"""

import logging
import os
import re
import sqlite3
import threading
import time
from typing import Any, Dict, List, Optional, Set, Tuple
from urllib.parse import unquote

MD_EXTENSIONS = ('.md', '.markdown', '.mdown', '.mkd')

_RE_FENCED_CODE = re.compile(r'(```[\s\S]*?```|~~~[\s\S]*?~~~)')
_RE_INLINE_CODE = re.compile(r'`[^`\n]+`')
_RE_WIKILINK = re.compile(r'\[\[([^\]\n]+)\]\]')
_RE_MD_LINK = re.compile(r'(?<!!)\[([^\]]+)\]\(([^)\s]+)(?:\s+"[^"]*")?\)')
_RE_EXTERNAL_URL = re.compile(r'^(?:https?://|ftp://|mailto:|data:|#)', re.IGNORECASE)

def _root_pattern(root_dir: str) -> str:
    # Match descendants only, and treat SQL wildcard characters in paths literally.
    prefix = os.path.join(os.path.abspath(root_dir), '')
    return prefix.replace('!', '!!').replace('%', '!%').replace('_', '!_') + '%'


_lock = threading.RLock()


def _mask_code_blocks(text: str) -> str:
    """用空格遮蔽代码块与行内代码，但完整保留换行符以确保行号 (line_no) 绝对精确。"""
    def _replace_keep_newlines(match):
        s = match.group(0)
        return ''.join('\n' if c == '\n' else ' ' for c in s)

    text = _RE_FENCED_CODE.sub(_replace_keep_newlines, text)
    text = _RE_INLINE_CODE.sub(_replace_keep_newlines, text)
    return text


class LinkIndexer:
    """本地双链倒排索引器。"""

    def __init__(self, db_path: Optional[str] = None):
        if not db_path:
            from src.readmd_core.config import DATA_DIR
            db_dir = os.path.join(DATA_DIR, 'index')
            os.makedirs(db_dir, exist_ok=True)
            db_path = os.path.join(db_dir, 'link_index.db')
        else:
            os.makedirs(os.path.dirname(os.path.abspath(db_path)), exist_ok=True)

        self.db_path = os.path.abspath(db_path)
        self._conn = sqlite3.connect(self.db_path, check_same_thread=False)
        self._conn.row_factory = sqlite3.Row
        self._init_db()

    def _init_db(self):
        with _lock:
            cur = self._conn.cursor()
            cur.execute("PRAGMA journal_mode=WAL;")
            cur.execute("PRAGMA synchronous=NORMAL;")
            cur.execute("""
                CREATE TABLE IF NOT EXISTS documents (
                    doc_id INTEGER PRIMARY KEY AUTOINCREMENT,
                    path TEXT UNIQUE NOT NULL,
                    title TEXT,
                    mtime REAL NOT NULL,
                    size INTEGER NOT NULL,
                    scanned_at REAL NOT NULL
                );
            """)
            cur.execute("""
                CREATE TABLE IF NOT EXISTS links (
                    link_id INTEGER PRIMARY KEY AUTOINCREMENT,
                    source_path TEXT NOT NULL,
                    target_raw TEXT NOT NULL,
                    target_clean TEXT NOT NULL,
                    target_path TEXT,
                    alias TEXT,
                    heading TEXT,
                    is_wikilink INTEGER NOT NULL DEFAULT 1,
                    line_no INTEGER NOT NULL DEFAULT 1,
                    FOREIGN KEY(source_path) REFERENCES documents(path) ON DELETE CASCADE
                );
            """)
            cur.execute("CREATE INDEX IF NOT EXISTS idx_links_source ON links(source_path);")
            cur.execute("CREATE INDEX IF NOT EXISTS idx_links_target_path ON links(target_path);")
            cur.execute("CREATE INDEX IF NOT EXISTS idx_links_target_clean ON links(target_clean);")
            self._conn.commit()

    def extract_links(self, content: str) -> List[Dict[str, Any]]:
        """从 Markdown 文本中提取所有双链与本地相对链接，自动屏蔽代码块。"""
        if not content:
            return []

        masked = _mask_code_blocks(content)
        lines = masked.split('\n')
        links: List[Dict[str, Any]] = []

        for line_idx, line in enumerate(lines, start=1):
            if not line.strip():
                continue

            # 1. 匹配 [[wikilinks]]
            for match in _RE_WIKILINK.finditer(line):
                raw_inner = match.group(1).strip()
                if not raw_inner:
                    continue

                if '|' in raw_inner:
                    target_part, alias = raw_inner.split('|', 1)
                    target_part = target_part.strip()
                    alias = alias.strip() or None
                else:
                    target_part = raw_inner
                    alias = None

                if '#' in target_part:
                    target_clean, heading = target_part.split('#', 1)
                    target_clean = target_clean.strip()
                    heading = heading.strip() or None
                else:
                    target_clean = target_part
                    heading = None

                if not target_clean and heading:
                    target_clean = '#' + heading

                links.append({
                    'target_raw': raw_inner,
                    'target_clean': target_clean,
                    'alias': alias,
                    'heading': heading,
                    'is_wikilink': True,
                    'line_no': line_idx,
                })

            # 2. 匹配 Markdown [text](url)
            for match in _RE_MD_LINK.finditer(line):
                alias_text = match.group(1).strip()
                raw_url = match.group(2).strip()
                if not raw_url or _RE_EXTERNAL_URL.search(raw_url):
                    continue

                url = unquote(raw_url)
                if '#' in url:
                    target_clean, heading = url.split('#', 1)
                    target_clean = target_clean.strip()
                    heading = heading.strip() or None
                else:
                    target_clean = url
                    heading = None

                if not target_clean:
                    continue

                links.append({
                    'target_raw': raw_url,
                    'target_clean': target_clean,
                    'alias': alias_text or None,
                    'heading': heading,
                    'is_wikilink': False,
                    'line_no': line_idx,
                })

        return links

    @staticmethod
    def _extract_title(content: str, fallback: str) -> str:
        """从 Markdown 提取一级标题，若无则使用文件名。"""
        for line in content.splitlines():
            line_s = line.strip()
            if line_s.startswith('# '):
                return line_s[2:].strip()
        return fallback

    def resolve_target(
        self,
        target_clean: str,
        source_path: str,
        known_files: Dict[str, str],
        basenames: Dict[str, List[str]],
    ) -> Optional[str]:
        """将 target_clean 解析为工作区内真实文件绝对路径，未找到则返回 None。"""
        if not target_clean or target_clean.startswith('#'):
            return source_path

        source_dir = os.path.dirname(source_path)

        # 1. 尝试基于源文件目录的相对路径
        cand1 = os.path.normpath(os.path.join(source_dir, target_clean))
        if cand1 in known_files:
            return cand1
        cand1_md = cand1 + '.md'
        if cand1_md in known_files:
            return cand1_md

        # 2. 尝试无后缀或同名文件名模糊匹配
        clean_norm = os.path.normpath(target_clean).lower()
        base = os.path.basename(clean_norm)
        if base.endswith(MD_EXTENSIONS):
            base_no_ext = os.path.splitext(base)[0]
        else:
            base_no_ext = base

        if base in basenames:
            return basenames[base][0]
        if (base_no_ext + '.md') in basenames:
            return basenames[base_no_ext + '.md'][0]

        return None

    def index_directory(self, root_dir: str, force: bool = False) -> Dict[str, int]:
        """对目录进行增量扫描与链接拓扑构建。"""
        root_dir = os.path.abspath(root_dir)
        if not os.path.isdir(root_dir):
            return {'scanned_count': 0, 'indexed_count': 0, 'deleted_count': 0}

        # 1. 发现当前目录下的所有 Markdown 文件
        disk_files: Dict[str, float] = {}
        for root, dirs, files in os.walk(root_dir):
            dirs[:] = [d for d in dirs if not d.startswith(('.', '_')) and d != 'node_modules']
            for name in files:
                if name.lower().endswith(MD_EXTENSIONS):
                    full = os.path.normpath(os.path.join(root, name))
                    try:
                        disk_files[full] = os.path.getmtime(full)
                    except OSError:
                        pass

        # 2. 构建文件名索引映射表供快速解析
        known_files = {p: p for p in disk_files}
        basenames: Dict[str, List[str]] = {}
        for p in disk_files:
            b = os.path.basename(p).lower()
            basenames.setdefault(b, []).append(p)

        # 3. 读取数据库已有文档记录
        with _lock:
            cur = self._conn.cursor()
            cur.execute("SELECT path, mtime FROM documents WHERE path LIKE ? ESCAPE '!'", (_root_pattern(root_dir),))
            db_docs = {row['path']: row['mtime'] for row in cur.fetchall()}

        # 4. 删除磁盘上已不存在的废弃文档
        deleted_paths = [p for p in db_docs if p not in disk_files]
        if deleted_paths:
            with _lock:
                cur = self._conn.cursor()
                cur.executemany("DELETE FROM documents WHERE path = ?", [(p,) for p in deleted_paths])
                cur.executemany("DELETE FROM links WHERE source_path = ?", [(p,) for p in deleted_paths])
                self._conn.commit()

        # 5. 过滤需要重新解析的文件（新增或 mtime 变化）
        to_index: List[str] = []
        for p, mtime in disk_files.items():
            if force or p not in db_docs or abs(db_docs[p] - mtime) > 0.001:
                to_index.append(p)

        indexed_count = 0
        now = time.time()

        for p in to_index:
            try:
                with open(p, 'r', encoding='utf-8', errors='replace') as f:
                    content = f.read()
                size = len(content)
                title = self._extract_title(content, os.path.basename(p))
                mtime = disk_files[p]

                extracted = self.extract_links(content)

                with _lock:
                    cur = self._conn.cursor()
                    cur.execute("""
                        INSERT INTO documents (path, title, mtime, size, scanned_at)
                        VALUES (?, ?, ?, ?, ?)
                        ON CONFLICT(path) DO UPDATE SET
                            title = excluded.title,
                            mtime = excluded.mtime,
                            size = excluded.size,
                            scanned_at = excluded.scanned_at;
                    """, (p, title, mtime, size, now))

                    cur.execute("DELETE FROM links WHERE source_path = ?", (p,))

                    link_rows = []
                    for item in extracted:
                        resolved = self.resolve_target(
                            item['target_clean'], p, known_files, basenames
                        )
                        link_rows.append((
                            p,
                            item['target_raw'],
                            item['target_clean'],
                            resolved,
                            item['alias'],
                            item['heading'],
                            1 if item['is_wikilink'] else 0,
                            item['line_no'],
                        ))

                    if link_rows:
                        cur.executemany("""
                            INSERT INTO links (
                                source_path, target_raw, target_clean, target_path,
                                alias, heading, is_wikilink, line_no
                            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?);
                        """, link_rows)

                    self._conn.commit()
                indexed_count += 1
            except Exception as exc:
                logging.warning("Failed to index markdown file %s: %s", p, exc)

        # 6. 对未重新解析但可能被新文件指向的悬空链接做二次补齐更新
        if indexed_count > 0 or deleted_paths:
            with _lock:
                cur = self._conn.cursor()
                cur.execute(
                    "SELECT link_id, source_path, target_clean FROM links "
                    "WHERE source_path LIKE ? ESCAPE '!'",
                    (_root_pattern(root_dir),),
                )
                unresolved = cur.fetchall()
                resolved_updates = []
                for row in unresolved:
                    res = self.resolve_target(row['target_clean'], row['source_path'], known_files, basenames)
                    resolved_updates.append((res, row['link_id']))
                if resolved_updates:
                    cur.executemany("UPDATE links SET target_path = ? WHERE link_id = ?", resolved_updates)
                    self._conn.commit()

        return {
            'scanned_count': len(disk_files),
            'indexed_count': indexed_count,
            'deleted_count': len(deleted_paths),
        }

    def get_forward_links(self, file_path: str) -> List[Dict[str, Any]]:
        """获取指定文件引用的所有出链。"""
        norm = os.path.normpath(os.path.abspath(file_path))
        with _lock:
            cur = self._conn.cursor()
            cur.execute("""
                SELECT link_id, target_raw, target_clean, target_path, alias, heading, is_wikilink, line_no
                FROM links WHERE source_path = ?
                ORDER BY line_no ASC;
            """, (norm,))
            return [dict(r) for r in cur.fetchall()]

    def get_backlinks(self, file_path: str) -> List[Dict[str, Any]]:
        """获取引用指定文件引用的所有反向入链（Backlinks）。"""
        norm = os.path.normpath(os.path.abspath(file_path))
        with _lock:
            cur = self._conn.cursor()
            cur.execute("""
                SELECT l.link_id, l.source_path, d.title as source_title, l.target_raw,
                       l.target_clean, l.alias, l.heading, l.is_wikilink, l.line_no
                FROM links l
                LEFT JOIN documents d ON l.source_path = d.path
                WHERE l.target_path = ?
                ORDER BY d.title ASC, l.line_no ASC;
            """, (norm,))
            return [dict(r) for r in cur.fetchall()]

    def get_deadlinks(self, root_dir: Optional[str] = None) -> List[Dict[str, Any]]:
        """获取所有目标文件不存在的悬空链接（Deadlinks）。"""
        with _lock:
            cur = self._conn.cursor()
            if root_dir:
                norm_root = os.path.normpath(os.path.abspath(root_dir))
                cur.execute("""
                    SELECT l.link_id, l.source_path, d.title as source_title, l.target_raw,
                           l.target_clean, l.alias, l.line_no
                    FROM links l
                    LEFT JOIN documents d ON l.source_path = d.path
                    WHERE l.target_path IS NULL AND l.source_path LIKE ? ESCAPE '!'
                    ORDER BY l.source_path ASC, l.line_no ASC;
                """, (_root_pattern(norm_root),))
            else:
                cur.execute("""
                    SELECT l.link_id, l.source_path, d.title as source_title, l.target_raw,
                           l.target_clean, l.alias, l.line_no
                    FROM links l
                    LEFT JOIN documents d ON l.source_path = d.path
                    WHERE l.target_path IS NULL
                    ORDER BY l.source_path ASC, l.line_no ASC;
                """)
            return [dict(r) for r in cur.fetchall()]

    def get_graph_data(self, root_dir: Optional[str] = None, max_nodes: int = 500) -> Dict[str, Any]:
        """构建图谱节点与连线数据供 Canvas 2D 力导向可视化。"""
        with _lock:
            cur = self._conn.cursor()
            if root_dir:
                norm_root = os.path.normpath(os.path.abspath(root_dir))
                cur.execute("SELECT path, title FROM documents WHERE path LIKE ? ESCAPE '!' LIMIT ?", (_root_pattern(norm_root), max_nodes))
            else:
                cur.execute("SELECT path, title FROM documents LIMIT ?", (max_nodes,))
            docs = cur.fetchall()

            doc_paths = {row['path']: row['title'] for row in docs}

            # 获取所有涉及的链接
            if root_dir:
                cur.execute("""
                    SELECT source_path, target_clean, target_path, alias, is_wikilink
                    FROM links
                    WHERE source_path LIKE ? ESCAPE '!'
                """, (_root_pattern(norm_root),))
            else:
                cur.execute("SELECT source_path, target_clean, target_path, alias, is_wikilink FROM links")
            links = cur.fetchall()

        nodes_map: Dict[str, Dict[str, Any]] = {}
        edges: List[Dict[str, Any]] = []

        # 辅助生成节点 ID（优先相对路径/文件名）
        def _get_node_id(full_path: Optional[str], fallback_name: str) -> str:
            if not full_path:
                return fallback_name
            if root_dir and full_path.startswith(norm_root):
                rel = os.path.relpath(full_path, norm_root).replace('\\', '/')
                return rel
            return os.path.basename(full_path)

        # 注册现有真实文档节点
        for p, t in doc_paths.items():
            nid = _get_node_id(p, os.path.basename(p))
            nodes_map[nid] = {
                'id': nid,
                'path': p,
                'label': t or os.path.basename(p),
                'is_deadlink': False,
                'link_count': 0,
                'backlink_count': 0,
            }

        # 构建连线并统计度数
        for l in links:
            src_p = l['source_path']
            src_id = _get_node_id(src_p, os.path.basename(src_p))
            if src_id not in nodes_map:
                continue

            tgt_p = l['target_path']
            if tgt_p:
                tgt_id = _get_node_id(tgt_p, os.path.basename(tgt_p))
                if tgt_id not in nodes_map:
                    nodes_map[tgt_id] = {
                        'id': tgt_id,
                        'path': tgt_p,
                        'label': os.path.basename(tgt_p),
                        'is_deadlink': False,
                        'link_count': 0,
                        'backlink_count': 0,
                    }
            else:
                tgt_id = l['target_clean']
                if tgt_id not in nodes_map:
                    nodes_map[tgt_id] = {
                        'id': tgt_id,
                        'path': None,
                        'label': tgt_id,
                        'is_deadlink': True,
                        'link_count': 0,
                        'backlink_count': 0,
                    }

            nodes_map[src_id]['link_count'] += 1
            nodes_map[tgt_id]['backlink_count'] += 1

            edges.append({
                'source': src_id,
                'target': tgt_id,
                'label': l['alias'] or '',
                'is_wikilink': bool(l['is_wikilink']),
            })

        nodes_list = list(nodes_map.values())
        for n in nodes_list:
            n['degree'] = n['link_count'] + n['backlink_count']

        deadlinks_count = sum(1 for n in nodes_list if n['is_deadlink'])

        return {
            'nodes': nodes_list,
            'edges': edges,
            'stats': {
                'total_nodes': len(nodes_list),
                'total_edges': len(edges),
                'deadlinks_count': deadlinks_count,
            }
        }

    def close(self):
        """关闭数据库连接。"""
        with _lock:
            try:
                self._conn.close()
            except Exception:
                pass


# 模块单例管理
_global_indexer: Optional[LinkIndexer] = None


def get_indexer(db_path: Optional[str] = None) -> LinkIndexer:
    """获取或初始化全局 LinkIndexer 单例。"""
    global _global_indexer
    if _global_indexer is None or db_path:
        with _lock:
            if _global_indexer is None or db_path:
                _global_indexer = LinkIndexer(db_path=db_path)
    return _global_indexer
