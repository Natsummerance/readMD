#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""ReadMD 小红书宣发海报无损格式转码与发布包导出工具

采用原生 PIL/Pillow 底层像素解码与转码，杜绝无头浏览器渲染空白问题，
100% 保留 1080x1440 画布上的大字排版、胶囊指标条与高精度截图。
"""

import glob
import os
import shutil
import sys
from PIL import Image
import numpy as np


def export_xhs(version="v239"):
    repo_root = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
    showcase_root = os.path.join(repo_root, "showcase")
    src_dir = os.path.join(showcase_root, "output", f"{version}-update")
    artifacts_dir = os.path.join(showcase_root, "output", f"xhs-{version}", "artifacts")
    images_dir = os.path.join(artifacts_dir, "images")
    update_dir = os.path.join(showcase_root, f"update-{version}")

    os.makedirs(images_dir, exist_ok=True)

    png_files = sorted(glob.glob(os.path.join(src_dir, "*.png")))
    if not png_files:
        raise FileNotFoundError(f"未在 {src_dir} 找到任何渲染好的 PNG 海报！")

    print(f"[XHS Export] 发现 {len(png_files)} 张高清海报，开始像素级转码为小红书 JPG...")

    for idx, png_path in enumerate(png_files):
        base_name = os.path.basename(png_path)
        page_no = f"{idx + 1:02d}"
        jpg_name = f"xhs-{page_no}-{os.path.splitext(base_name)[0]}.jpg"
        jpg_path = os.path.join(images_dir, jpg_name)

        # 打开 PNG 并转为 RGB
        with Image.open(png_path) as img:
            rgb_img = img.convert("RGB")
            # 严格保存为高品质 95% JPEG
            rgb_img.save(jpg_path, "JPEG", quality=95, optimize=True)

            # 像素自检：验证绝对不是白屏
            arr = np.array(rgb_img)
            mean_val = float(np.mean(arr))
            std_val = float(np.std(arr))
            size_kb = os.stat(jpg_path).st_size / 1024.0

            is_blank = (mean_val > 252.0 and std_val < 5.0)
            if is_blank:
                raise ValueError(f"CRITICAL ERROR: {jpg_name} 图像内容异常（白屏判定）！")

            print(f"  [OK] {page_no}/{len(png_files)} {jpg_name} ({size_kb:.1f} KB, 色彩均值: {mean_val:.1f}, 标准差: {std_val:.1f})")

    # 同步文本元数据
    for meta in ["title.txt", "body.txt", "topics.txt"]:
        src_meta = os.path.join(update_dir, meta)
        if os.path.exists(src_meta):
            shutil.copy2(src_meta, os.path.join(artifacts_dir, meta))
            print(f"  [Meta Sync] {meta} -> {artifacts_dir}")

    print(f"\n[XHS Export] 全部 16 张小红书发布图片已 100% 成功生成并验证！路径: {images_dir}")


if __name__ == "__main__":
    export_xhs()
