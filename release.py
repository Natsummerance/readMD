# -*- coding: utf-8 -*-
"""发布 ReadMD 到 GitHub Releases（无需 gh CLI，纯标准库）。

用法：
    python release.py --tag v1.4.0 [--draft] [--skip-assets]

需要环境变量 GITHUB_TOKEN（可存系统变量）。
"""
import argparse
import json
import os
import sys
import urllib.request
import urllib.error

OWNER = "Natsummerance"
REPO = "readMD"
API = "https://api.github.com/repos/%s/%s" % (OWNER, REPO)
UPLOAD = "https://uploads.github.com/repos/%s/%s" % (OWNER, REPO)


def token():
    t = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    if not t:
        raise SystemExit("缺少 GITHUB_TOKEN 环境变量")
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
        with urllib.request.urlopen(req, timeout=600) as r:
            raw = r.read()
            return r.status, raw
    except urllib.error.HTTPError as e:
        return e.code, e.read()


def get_release(tag, tok):
    st, raw = api("GET", "%s/releases/tags/%s" % (API, tag), token_str=tok)
    if st == 200:
        return json.loads(raw)
    return None


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
    ap = argparse.ArgumentParser(description="发布 ReadMD Release")
    ap.add_argument("--tag", default="v1.4.0")
    ap.add_argument("--name", default="ReadMD v1.4.0")
    ap.add_argument("--body-file")
    ap.add_argument("--draft", action="store_true")
    ap.add_argument("--skip-assets", action="store_true")
    args = ap.parse_args()

    tok = token()
    import urllib.parse  # noqa
    base = os.path.dirname(os.path.abspath(__file__))
    assets = [
        (os.path.join(base, "dist", "ReadMDSetup.exe"), "ReadMDSetup-v%s.exe" % args.tag.lstrip("v")),
        (os.path.join(base, "dist", "ReadMD.exe"), "ReadMD-portable-v%s.exe" % args.tag.lstrip("v")),
        (os.path.join(base, "assets", "icon-256.png"), "readmd-icon-256.png"),
    ]
    for fp, _ in assets:
        if not os.path.isfile(fp):
            raise SystemExit("缺少文件: %s（请先打包）" % fp)

    rel = get_release(args.tag, tok)
    if rel is None:
        body = args.body_file and open(args.body_file, encoding="utf-8").read() or DEFAULT_BODY
        st, raw = api("POST", "%s/releases" % API,
                      {"tag_name": args.tag, "name": args.name, "body": body,
                       "draft": args.draft, "prerelease": False}, token_str=tok)
        if st not in (200, 201):
            raise SystemExit("创建 Release 失败 (%s): %s" % (st, raw.decode("utf-8", "replace")))
        rel = json.loads(raw)
        print("release created: %s" % rel["html_url"])
    else:
        print("release already exists: %s" % rel["html_url"])

    if args.skip_assets:
        return 0
    existing = {a["name"] for a in rel.get("assets", [])}
    ok = True
    for fp, name in assets:
        if name in existing:
            print("asset exists, skip: %s" % name)
            continue
        if not upload_asset(rel["id"], fp, name, tok):
            ok = False
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
