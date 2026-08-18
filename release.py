# -*- coding: utf-8 -*-
"""发布 ReadMD 到 GitHub Releases（无需 gh CLI，纯标准库）。

用法：
    python release.py --verify [--tag v2.2.3]        # 校验 CI 创建的四资产 Release
    python release.py --update [--tag v2.2.3]        # 仅维护已存在 Release 的文案

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
    """v2.2.3 统一发布的 Windows 与 macOS 四个资产。"""
    v = tag.lstrip("v")
    return [
        (os.path.join(base, "dist", "ReadMDSetup-v%s.exe" % v), "ReadMDSetup-v%s.exe" % v),
        (os.path.join(base, "dist", "ReadMD-portable-v%s.exe" % v), "ReadMD-portable-v%s.exe" % v),
        (os.path.join(base, "ReadMD-macos-x64-v%s.zip" % v), "ReadMD-macos-x64-v%s.zip" % v),
        (os.path.join(base, "ReadMD-macos-arm64-v%s.zip" % v), "ReadMD-macos-arm64-v%s.zip" % v),
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
    """校验线上四资产；本地产物存在时额外逐字节比对。"""
    base = os.path.dirname(os.path.abspath(__file__))
    assets = build_assets(base, args.tag)
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
        digest = a.get("digest", "")
        remote_ok = a.get("state") == "uploaded" and a.get("size", 0) > 0 and digest.startswith("sha256:")
        if os.path.isfile(fp):
            size_ok = a["size"] == os.path.getsize(fp)
            sha_ok = digest == "sha256:" + sha256(fp)
        else:
            size_ok = sha_ok = True
        status = "OK" if (remote_ok and size_ok and sha_ok) else "DIFF"
        if status != "OK":
            ok = False
        print("%-7s %s  size:%s/%s  sha256:%s" % (
            status, name,
            a["size"], os.path.getsize(fp) if os.path.isfile(fp) else "remote-only",
            "MATCH" if sha_ok and os.path.isfile(fp) else ("REMOTE" if sha_ok else "MISMATCH")))
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

> 安装包（推荐）：下载 **ReadMDSetup-v2.2.3.exe**。
> 便携版：下载 **ReadMD-portable-v2.2.3.exe**。
> macOS：按芯片下载 x64 或 arm64 ZIP；当前版本未签名，首次打开请在 Finder 中右键选择“打开”。

## v2.2.3

- 修复 PDF、DOCX、HTML 导出的保存路径类型错误，并改为原子写入。
- 网页转 Markdown 增加离线 Defuddle、交互式 WebView、短内容识别和显式内网页面授权。
- 顶栏文件名支持点击或 F2 直接重命名当前本地 Markdown 文件。
- Windows、Intel macOS 和 Apple Silicon macOS 由同一 GitHub Actions 工作流发布。
"""


def main():
    ap = argparse.ArgumentParser(description="发布 ReadMD Release（纯标准库，无需 gh CLI）")
    ap.add_argument("--tag", default="v2.2.3")
    ap.add_argument("--name", default="ReadMD v2.2.3")
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

    if args.update:
        update_release(args, tok)
        return 0
    raise SystemExit("Release 仅由 GitHub Actions 创建；请使用 --verify，或用 --update 维护文案")


if __name__ == "__main__":
    sys.exit(main())
