# -*- coding: utf-8 -*-
"""维护由 GitHub Actions 创建的 ReadMD v2.2.6 Release。

用法：
    python release.py --verify [--tag v2.2.6]  # 校验既有 Release 的五个资产
    python release.py --update [--tag v2.2.6]  # 仅更新既有 Release 的标题/说明

本工具不会创建 Release、创建标签或上传/删除资产；发布只由 release.yml 的
``v2.2.6`` tag job 执行。
"""
import argparse
import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.request

OWNER = "Natsummerance"
REPO = "readMD"
API = "https://api.github.com/repos/%s/%s" % (OWNER, REPO)


def token():
    value = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    if not value:
        raise SystemExit("缺少 GITHUB_TOKEN 或 GH_TOKEN 环境变量")
    return value


def api(method, url, body=None, token_str=None, retries=3):
    headers = {"Accept": "application/vnd.github+json", "User-Agent": "readmd-release"}
    if token_str:
        headers["Authorization"] = "Bearer " + token_str
    data = json.dumps(body).encode("utf-8") if body is not None else None
    request = urllib.request.Request(url, data=data, method=method, headers=headers)
    last = None
    for attempt in range(retries + 1):
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return response.status, response.read()
        except urllib.error.HTTPError as exc:
            return exc.code, exc.read()
        except Exception as exc:  # transient network failure
            last = exc
            if attempt < retries:
                time.sleep(2 * (attempt + 1))
    raise last


def get_release(tag, tok):
    status, raw = api("GET", "%s/releases/tags/%s" % (API, tag), token_str=tok)
    return json.loads(raw) if status == 200 else None


def sha256(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def asset_names(tag):
    version = tag.lstrip("v")
    return [
        "ReadMDSetup-v%s.exe" % version,
        "ReadMD-portable-v%s.exe" % version,
        "ReadMD-macos-x64-v%s.zip" % version,
        "ReadMD-macos-arm64-v%s.zip" % version,
        "SHA256SUMS.txt",
    ]


def local_path(base, name):
    candidates = [os.path.join(base, "dist", name), os.path.join(base, name)]
    return next((candidate for candidate in candidates if os.path.isfile(candidate)), None)


def verify_release(args, tok):
    release = get_release(args.tag, tok)
    if release is None:
        print("VERIFY FAILED: Release %s 不存在（此工具不会创建它）" % args.tag)
        return 1
    remote = {asset["name"]: asset for asset in release.get("assets", [])}
    base = os.path.dirname(os.path.abspath(__file__))
    passed = True
    for name in asset_names(args.tag):
        asset = remote.get(name)
        if asset is None:
            print("MISSING-REMOTE %s" % name)
            passed = False
            continue
        ready = asset.get("state") == "uploaded" and asset.get("size", 0) > 0
        digest = asset.get("digest", "")
        digest_ok = digest.startswith("sha256:")
        path = local_path(base, name)
        local_ok = True
        if path:
            local_ok = asset["size"] == os.path.getsize(path) and digest == "sha256:" + sha256(path)
        status = "OK" if ready and digest_ok and local_ok else "DIFF"
        print("%-7s %s  size=%s  sha256=%s" % (
            status, name, asset.get("size", 0), "MATCH" if path and local_ok else
            ("REMOTE" if digest_ok else "MISSING")))
        passed = passed and status == "OK"
    print("VERIFY %s" % ("PASSED" if passed else "FAILED"))
    return 0 if passed else 1


def update_release(args, tok):
    release = get_release(args.tag, tok)
    if release is None:
        raise SystemExit("Release %s 不存在；只能由 GitHub Actions 创建" % args.tag)
    patch = {"name": args.name}
    if args.body_file:
        with open(args.body_file, encoding="utf-8") as handle:
            patch["body"] = handle.read()
    status, raw = api("PATCH", "%s/releases/%s" % (API, release["id"]), patch, tok)
    if status != 200:
        raise SystemExit("更新 Release 失败 (%s): %s" % (status, raw.decode("utf-8", "replace")))
    print("release updated: %s" % json.loads(raw)["html_url"])


def main():
    parser = argparse.ArgumentParser(description="维护既有 ReadMD Release")
    parser.add_argument("--tag", default="v2.2.6")
    parser.add_argument("--name", default="ReadMD v2.2.6")
    parser.add_argument("--body-file")
    parser.add_argument("--verify", action="store_true", help="校验既有 Release 的五个资产")
    parser.add_argument("--update", action="store_true", help="更新既有 Release 的标题/说明")
    args = parser.parse_args()
    if args.verify == args.update:
        parser.error("请且只能指定 --verify 或 --update")
    tok = token()
    if args.verify:
        return verify_release(args, tok)
    update_release(args, tok)
    return 0


if __name__ == "__main__":
    sys.exit(main())
