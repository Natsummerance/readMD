# -*- coding: utf-8 -*-
"""发布 ReadMD 到 GitHub Releases（无需 gh CLI，纯标准库）。

用法：
    python release.py --verify [--tag v1.4.0]        # 校验线上 Release 与本地产物（名称/大小/SHA256）
    python release.py --update [--tag v1.4.0]        # 更新已存在 Release 的标题与说明
    python release.py [--tag v1.4.0]                 # 创建 Release（已存在则跳过），上传缺失资产
    python release.py --force-upload [--tag v1.4.0]  # 强制重传全部资产（覆盖同名）

通用参数：--name 标题  --body-file 说明文件  --draft  --skip-assets  --asset 指定单个资产名
需要环境变量 GITHUB_TOKEN（可存系统变量）。
"""
import argparse
import hashlib
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request

OWNER = "Natsummerance"
REPO = "readMD"
API = "https://api.github.com/repos/%s/%s" % (OWNER, REPO)
UPLOAD = "https://uploads.github.com/repos/%s/%s" % (OWNER, REPO)


def token():
    t = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    if not t:
        raise SystemExit("缺少 GITHUB_TOKEN 环境变量（可在系统变量中配置）")
    return t


def api(method, url, body=None, token_str=None, binary=False, headers=None):
    h = {"Accept": "application/vnd.github+json", "User-Agent": "readmd-release"}
    if token_str:
        h["Authorization"] = "Bearer " + token_str
    if headers:
        h.update(headers)
    data = body if binary else (json.dumps(body).encode("utf-8") if body is not None else None)
    req = urllib.request.Request(url, data=data, method=method, headers=h)
    try:
        with urllib.request.urlopen(req, timeout=900) as r:
            raw = r.read()
            return r.status, raw
    except urllib.error.HTTPError as e:
        return e.code, e.read()


def get_release(tag, tok):
    st, raw = api("GET", "%s/releases/tags/%s" % (API, tag), token_str=tok)
    if st == 200:
        return json.loads(raw)
    return None


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def build_assets(base, tag):
    v = tag.lstrip("v")
    return [
        (os.path.join(base, "dist", "ReadMDSetup.exe"), "ReadMDSetup-v%s.exe" % v),
        (os.path.join(base, "dist", "ReadMD.exe"), "ReadMD-portable-v%s.exe" % v),
        (os.path.join(base, "assets", "icon-256.png"), "readmd-icon-256.png"),
    ]


def upload_asset(rel_id, file_path, asset_name, tok):
    size = os.path.getsize(file_path)
    print("uploading %s (%d MB) ..." % (asset_name, size // 1048576), flush=True)
    with open(file_path, "rb") as f:
        data = f.read()
    url = "%s/releases/%s/assets?name=%s" % (UPLOAD, rel_id, urllib.parse.quote(asset_name))
    st, raw = api("POST", url, body=data, token_str=tok, binary=True,
                  headers={"Content-Type": "application/octet-stream"})
    if st in (200, 201):
        print("  uploaded OK: %s" % asset_name, flush=True)
        return True
    print("  upload FAILED (%s): %s" % (st, raw.decode("utf-8", "replace")[:300]), flush=True)
    return False


def ensure_release(args, tok):
    rel = get_release(args.tag, tok)
    if rel is not None:
        print("release already exists: %s" % rel["html_url"], flush=True)
        return rel
    body = DEFAULT_BODY
    if args.body_file:
        with open(args.body_file, encoding="utf-8") as f:
            body = f.read()
    st, raw = api("POST", "%s/releases" % API,
                  {"tag_name": args.tag, "name": args.name, "body": body,
                   "draft": args.draft, "prerelease": False}, token_str=tok)
    if st not in (200, 201):
        raise SystemExit("创建 Release 失败 (%s): %s" % (st, raw.decode("utf-8", "replace")))
    rel = json.loads(raw)
    print("release created: %s" % rel["html_url"], flush=True)
    return rel


def update_release(args, tok):
    rel = get_release(args.tag, tok)
    if rel is None:
        print("Release %s 不存在，改用创建模式..." % args.tag)
        return ensure_release(args, tok)
    patch = {"name": args.name}
    if args.body_file:
        with open(args.body_file, encoding="utf-8") as f:
            patch["body"] = f.read()
    st, raw = api("PATCH", "%s/releases/%s" % (API, rel["id"]), body=patch, token_str=tok)
    if st != 200:
        raise SystemExit("更新 Release 失败 (%s): %s" % (st, raw.decode("utf-8", "replace")))
    rel = json.loads(raw)
    print("release updated: %s" % rel["html_url"], flush=True)
    return rel


def verify_release(args, tok):
    """校验线上资产与本地产物一一对应（名称 / 大小 / SHA256）。"""
    base = os.path.dirname(os.path.abspath(__file__))
    assets = build_assets(base, args.tag)
    missing_local = [(fp, name) for fp, name in assets if not os.path.isfile(fp)]
    if missing_local:
        for fp, name in missing_local:
            print("MISSING-LOCAL  %s" % name)
        print("VERIFY FAILED: 本地缺少产物，请先打包")
        return 1
    rel = get_release(args.tag, tok)
    if rel is None:
        print("VERIFY FAILED: Release %s 不存在" % args.tag)
        return 1
    remote = {a["name"]: a for a in rel.get("assets", [])}
    ok = True
    for fp, name in assets:
        if name not in remote:
            print("MISSING-REMOTE %s" % name)
            ok = False
            continue
        a = remote[name]
        size_ok = a["size"] == os.path.getsize(fp)
        digest = a.get("digest", "")
        sha_ok = digest == "sha256:" + sha256(fp)
        status = "OK" if (size_ok and sha_ok) else "DIFF"
        if status != "OK":
            ok = False
        print("%-7s %s  size:%s/%s  sha256:%s" % (
            status, name,
            a["size"], os.path.getsize(fp),
            "MATCH" if sha_ok else "MISMATCH"))
    print("VERIFY %s" % ("PASSED" if ok else "FAILED"))
    return 0 if ok else 1


def upload_all(rel, args, tok):
    base = os.path.dirname(os.path.abspath(__file__))
    assets = build_assets(base, args.tag)
    for fp, name in assets:
        if not os.path.isfile(fp):
            raise SystemExit("缺少文件: %s（请先打包）" % fp)
    existing = {a["name"] for a in rel.get("assets", [])}
    ok = True
    for fp, name in assets:
        if args.asset and name != args.asset:
            continue
        if name in existing and not args.force_upload:
            print("asset exists, skip: %s" % name)
            continue
        if name in existing and args.force_upload:
            for a in rel.get("assets", []):
                if a["name"] == name:
                    st, raw = api("DELETE", "%s/releases/assets/%s" % (API, a["id"]), token_str=tok)
                    if st != 204:
                        print("  delete old asset FAILED (%s): %s" % (st, raw.decode("utf-8", "replace")[:200]))
                        ok = False
                    break
        if not upload_asset(rel["id"], fp, name, tok):
            ok = False
    return 0 if ok else 1


DEFAULT_BODY = """## 下载

> 安装包（推荐）：下载 **ReadMDSetup-v1.4.0.exe**，双击即可安装（内含炫酷动画安装界面，支持设为 .md 默认打开方式，可随时在「设置 → 应用」中卸载）。
> 便携版：**ReadMD-portable-v1.4.0.exe** 免安装，双击直接运行。

## 新增

- 编辑器插入图片：裁剪 / 缩放 / 旋转（Canvas 所见即所得，保存到文档 `images/` 目录）
- 全新安装器：苹果风格动画 UI（毛玻璃、弹簧动效、极光背景），一键安装 / 升级 / 卸载

## 完整功能

- 秒开渲染：先渲染 Markdown，后台懒加载转换 / OCR / 网页 / AI 模块
- 自动修正：表格、加粗、公式、标题等常见错误（只影响显示，不改原文件）
- AI 助手：15+ 提供商预设 + 自定义，快速阅读 / 修改 / 扩充 / 续写 / 润色，Prompt 模板，历史会话
- 万物转 MD：docx / pptx / xlsx / pdf / html 等（MarkItDown）
- 扫描转 MD：Windows 内置 OCR（离线、支持中文）
- 网页转 MD：URL 抓取正文，可批量爬取
- 主动编辑：CodeMirror 6 语法高亮 + 自动补全 + 工具栏插入，Ctrl+S 保存
- 移动端共享：手机扫码在同一 Wi-Fi 阅读
- 大文档增量流式渲染、三主题、目录、搜索、打印导出 PDF
"""


def main():
    ap = argparse.ArgumentParser(description="发布 ReadMD Release（纯标准库，无需 gh CLI）")
    ap.add_argument("--tag", default="v1.4.0")
    ap.add_argument("--name", default="ReadMD v1.4.0")
    ap.add_argument("--body-file")
    ap.add_argument("--draft", action="store_true")
    ap.add_argument("--skip-assets", action="store_true")
    ap.add_argument("--force-upload", action="store_true", help="同名资产先删除再重传")
    ap.add_argument("--asset", help="只处理指定资产名")
    ap.add_argument("--verify", action="store_true", help="校验线上与本地产物一致性后退出")
    ap.add_argument("--update", action="store_true", help="更新已存在 Release 的标题/说明")
    args = ap.parse_args()

    tok = token()
    if args.verify:
        return verify_release(args, tok)

    rel = update_release(args, tok) if args.update else ensure_release(args, tok)
    if args.skip_assets:
        return 0
    return upload_all(rel, args, tok)


if __name__ == "__main__":
    sys.exit(main())
