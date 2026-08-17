# -*- coding: utf-8 -*-
"""发布 ReadMD 到 GitHub Releases（无需 gh CLI，纯标准库）。

用法：
    python release.py --verify [--tag v2.2.0]        # 校验 CI 创建的 Release（名称/大小/SHA256）
    python release.py --update [--tag v2.2.0]        # 必要时维护已存在 Release

通用参数：--name 标题  --body-file 说明文件  --draft  --skip-assets  --asset 指定单个资产名
需要环境变量 GITHUB_TOKEN（可存系统变量）。
"""
import argparse
import hashlib
import json
import os
import sys
import time
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


def api(method, url, body=None, token_str=None, binary=False, headers=None, retries=3):
    """请求 GitHub API；网络抖动自动重试（指数退避）。"""
    h = {"Accept": "application/vnd.github+json", "User-Agent": "readmd-release"}
    if token_str:
        h["Authorization"] = "Bearer " + token_str
    if headers:
        h.update(headers)
    data = body if binary else (json.dumps(body).encode("utf-8") if body is not None else None)
    req = urllib.request.Request(url, data=data, method=method, headers=h)
    last = None
    for i in range(retries + 1):
        try:
            with urllib.request.urlopen(req, timeout=900) as r:
                raw = r.read()
                return r.status, raw
        except urllib.error.HTTPError as e:
            return e.code, e.read()
        except Exception as e:  # 连接重置/超时等瞬时故障
            last = e
            if i < retries:
                print("  network error (%s), retry %d/%d ..." % (e, i + 1, retries), flush=True)
                time.sleep(2 * (i + 1))
    raise last


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
    """发布资产：安装包 + 便携版。安装版为 onedir 目录安装（秒开），便携版为单文件。"""
    v = tag.lstrip("v")
    return [
        (os.path.join(base, "dist", "ReadMDSetup.exe"), "ReadMDSetup-v%s.exe" % v),
        (os.path.join(base, "dist", "ReadMD-portable.exe"), "ReadMD-portable-v%s.exe" % v),
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
                   "draft": args.draft, "prerelease": bool(args.prerelease)}, token_str=tok)
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
    if args.asset_file:
        if len(args.asset_file) != len(args.asset_name or []):
            raise SystemExit("--asset-file 与 --asset-name 数量必须一致")
        assets = list(zip(args.asset_file, args.asset_name or []))
    else:
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

> 安装包（推荐）：下载 **ReadMDSetup-v2.2.0.exe**。
> 便携版：下载 **ReadMD-portable-v2.2.0.exe**。
> macOS：按芯片下载 x64 或 arm64 ZIP；当前版本未签名，首次打开请在 Finder 中右键选择“打开”。

## 修复（v2.0.1）

- 修复安装器黑屏弹窗：v2.0.0 安装包使用 PyInstaller 启动画面但未调用 pyi_splash.close()，黑色启动画面永远置顶且无法关闭，低配机安装时会被卡住。v2.0.1 移除启动画面并加入防御性关闭逻辑
- 安装版本号统一为 2.0.1

## 新增（v2.0.0）

- 秒开启动：安装版改为目录安装（不再每次解压 96MB 单文件），冷启动窗口可用 ≤1.5s；常驻托盘后双击 .md 再次打开 <0.3s
- 单实例常驻：关闭窗口进系统托盘，再打开文件瞬时唤起；托盘菜单「显示 / 打开文件 / 退出」
- 界面全面改版：现代清爽阅读风（44px 工具条 + 内联 SVG 图标、欢迎页最近文件网格、三主题全套设计 token）
- 阅读体验：标题层级 / 代码块 / 表格 / 引用 / 任务列表精修，目录滚动高亮，大文档骨架屏，动画遵循系统减弱动效设置

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
    ap.add_argument("--tag", default="v2.2.0")
    ap.add_argument("--name", default="ReadMD v2.2.0")
    ap.add_argument("--body-file")
    ap.add_argument("--draft", action="store_true")
    ap.add_argument("--skip-assets", action="store_true")
    ap.add_argument("--force-upload", action="store_true", help="同名资产先删除再重传")
    ap.add_argument("--prerelease", action="store_true", help="标记为预发布（Win7 Beta 等）")
    ap.add_argument("--asset", help="只处理指定资产名")
    ap.add_argument("--asset-file", action="append", help="自定义资产文件路径（可重复）")
    ap.add_argument("--asset-name", action="append", help="对应资产名称（与 --asset-file 配对）")
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
