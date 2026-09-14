# -*- coding: utf-8 -*-
import glob
import json
import os
import sys
import time
from pathlib import Path

sys.path.insert(0, r"C:\Users\Natsumer\.gemini\config\skills\xhs-publish\scripts")
import xhs_publish

def main():
    print("[Publish Pipeline] 开始连接 CDP 代理并定位发布页 Tab...")
    targets = xhs_publish.list_targets()
    target = None
    for t in targets:
        url = t.get("url") or ""
        if "/publish/publish" in url:
            target = t["targetId"]
            break

    if not target:
        raise RuntimeError("未找到处于 /publish/publish 的发布页 Tab！")

    print(f"[Publish Pipeline] 成功锁定目标 Tab: {target}")

    # 读取发布物料
    repo_root = Path(r"T:\Programming\Project\codex\creator\readmd")
    update_dir = repo_root / "showcase" / "update-v239"
    images_dir = repo_root / "showcase" / "output" / "xhs-v239" / "artifacts" / "images"

    title = (update_dir / "title.txt").read_text(encoding="utf-8").strip()
    body = (update_dir / "body.txt").read_text(encoding="utf-8").strip()
    topics_raw = (update_dir / "topics.txt").read_text(encoding="utf-8").strip().splitlines()
    topics = [t.strip() for t in topics_raw if t.strip()][:5]

    all_images = sorted(glob.glob(str(images_dir / "*.jpg")))
    if len(all_images) != 16:
        raise RuntimeError(f"预期 16 张海报，实际找到 {len(all_images)} 张！")

    # 检查当前已有图片数
    current_count = xhs_publish.eval_js(target, xhs_publish.JS_IMG_COUNT)
    if not isinstance(current_count, int) or current_count < 0:
        current_count = 0
    print(f"[Publish Pipeline] 当前已上传图片数: {current_count}/16")

    # 上传剩余图片
    for idx in range(current_count, len(all_images)):
        img_path = all_images[idx]
        target_no = idx + 1
        print(f"[Publish Pipeline] 正在上传第 {target_no}/16 张海报: {os.path.basename(img_path)}...")
        
        # 如果第一张还没上传（理论上应该已上传，但作防御）
        if idx == 0:
            xhs_publish.set_files(target, "input[type=file].upload-input", [img_path])
        else:
            xhs_publish.set_files(target, "input[type=file][accept*='.jpg']", [img_path])

        # 等待计数增长到 target_no
        for _ in range(30):
            time.sleep(1)
            n = xhs_publish.eval_js(target, xhs_publish.JS_IMG_COUNT)
            if isinstance(n, int) and n >= target_no:
                print(f"  [OK] 状态已更新为 {n}/18")
                break
        else:
            raise RuntimeError(f"上传第 {target_no} 张后未能检测到图片计数更新")
        time.sleep(1)

    print(f"[Publish Pipeline] 全部 16 张海报已 100% 上传就绪！")

    # 填写标题
    print(f"[Publish Pipeline] 填写标题 ({len(title)} 字): {title}")
    xhs_publish.fill_title(target, title)
    time.sleep(1)

    # 填写正文
    print(f"[Publish Pipeline] 填写正文 ({len(body)} 字)...")
    xhs_publish.fill_body(target, body)
    time.sleep(1)

    # 添加话题
    print(f"[Publish Pipeline] 挂载话题: {topics}")
    topic_results = xhs_publish.add_topics(target, topics)
    print(f"[Publish Pipeline] 话题添加结果: {topic_results}")
    time.sleep(2)

    # 预览自检
    preview = xhs_publish.eval_js(target, """
    (() => ({
      title: (document.querySelector('input[placeholder="填写标题会有更多赞哦"]')||{}).value || '',
      bodyLen: ((document.querySelector('.tiptap.ProseMirror')||{}).innerText || '').length,
      topics: Array.from(document.querySelectorAll('a.tiptap-topic')).map(a => (a.innerText||'').trim()),
      imgCount: (Array.from(document.querySelectorAll('div.status')).find(e => /^\\d+\\/18$/.test((e.innerText||'').trim()))||{}).innerText || ''
    }))()
    """)
    print(f"[Publish Pipeline] 发布前校验数据: {json.dumps(preview, ensure_ascii=False, indent=2)}")

    if not preview.get("title") or preview.get("imgCount") != "16/18":
        raise RuntimeError(f"发布前校验不通过！{preview}")

    # 点击发布并轮询跳转
    print(f"[Publish Pipeline] 正在触发最终发布动作 (CustomEvent('publish'))...")
    xhs_publish.do_publish(target, timeout=90)

    print("\n==================================================")
    print("🎉 [Publish Pipeline] 恭喜！小红书笔记发布成功！已成功跳转完成。")
    print("==================================================")

if __name__ == "__main__":
    main()
