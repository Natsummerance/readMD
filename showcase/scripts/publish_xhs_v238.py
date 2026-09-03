import argparse
import json
import os
import sys
import time
from pathlib import Path

# Add xhs-publish script to path
XHS_PUBLISH_DIR = Path.home() / ".gemini" / "config" / "skills" / "xhs-publish" / "scripts"
sys.path.insert(0, str(XHS_PUBLISH_DIR))

import xhs_publish

def main():
    parser = argparse.ArgumentParser(description="发布 ReadMD v2.3.8 小红书笔记")
    parser.add_argument("--draft", action="store_true", help="只填充不点击发布")
    parser.add_argument("--proxy", default="http://127.0.0.1:3456")
    parser.add_argument("--publish-timeout", type=int, default=90)
    args = parser.parse_args()

    artifacts_dir = Path(__file__).resolve().parents[1] / "output" / "xhs-v238" / "artifacts"
    metadata_path = artifacts_dir / "metadata.json"
    if not metadata_path.exists():
        raise FileNotFoundError(f"Missing {metadata_path}")

    meta = json.loads(metadata_path.read_text(encoding="utf-8"))
    title = meta["title"].strip()
    body = meta["body"].strip()
    topics = meta["topics"]
    images = [Path(img) for img in meta["images"]]

    for img in images:
        if not img.exists():
            raise FileNotFoundError(f"Image not found: {img}")

    print(f"[Publish] 准备发布笔记:")
    print(f"  标题: {title} ({len(title)} 字)")
    print(f"  字数: {len(body)} 字")
    print(f"  封面: {images[0].name}")
    print(f"  配图: {len(images)} 张")
    print(f"  话题: {topics}")
    print(f"  模式: {'草稿模式 (不点发布)' if args.draft else '正式全自动发布'}")

    # Set up arguments for xhs_publish.cmd_publish
    publish_args = argparse.Namespace(
        proxy=args.proxy,
        title=title,
        title_file=None,
        body=body,
        body_file=None,
        cover=str(images[0]),
        images=[str(img) for img in images[1:]],
        topics=topics,
        no_publish=args.draft,
        dry_run=False,
        force=False,
        publish_timeout=args.publish_timeout,
        bootstrap_edge=False,
        restart_edge=False
    )

    print("[Publish] 连接 CDP 代理并验证登录态...")
    xhs_publish.PROXY = args.proxy
    target = xhs_publish.open_publish_tab()
    xhs_publish.ensure_login(target)
    print(f"[Publish] 登录态有效，发布页面 Target: {target}")

    print("[Publish] 切换图文模式...")
    xhs_publish.ensure_image_mode(target)

    print(f"[Publish] 正在上传全部 {len(images)} 张 1080x1440 高清证据海报...")
    uploaded = xhs_publish.upload_images(target, [str(img) for img in images])
    print(f"[Publish] 已上传 {uploaded}/{len(images)} 张图片")

    print("[Publish] 填写标题...")
    xhs_publish.fill_title(target, title)

    print("[Publish] 填写正文...")
    xhs_publish.fill_body(target, body)

    print("[Publish] 逐项添加话题标签...")
    topic_results = xhs_publish.add_topics(target, topics)
    print(f"[Publish] 话题添加结果: {topic_results}")

    if args.draft:
        print("[Publish] 草稿模式已填充完毕，保留在发布界面供人工检阅。")
        return

    print("[Publish] 点击发布按钮...")
    xhs_publish.do_publish(target, timeout=args.publish_timeout)
    print("[Publish] 发布指令触发成功！正在等待页面跳转及审核状态同步...")

    time.sleep(5)
    print("[Publish] 查询发布审核状态...")
    try:
        status = xhs_publish.get_note_status(target, note_title=title)
        print(f"[Publish] 审核状态返回: {json.dumps(status, ensure_ascii=False, indent=2)}")
    except Exception as exc:
        print(f"[Publish] 状态轮询异常 (发布已完成): {exc}")

if __name__ == "__main__":
    main()
