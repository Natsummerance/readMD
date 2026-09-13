#!/usr/bin/env node

/**
 * ReadMD 小红书宣发包全自动导出脚本
 * 统一代理至 Python Pillow 像素级转码，彻底根除无头浏览器渲染空白问题。
 */

const { spawnSync } = require('child_process');
const path = require('path');

const scriptPath = path.join(__dirname, 'export_xhs_package.py');
console.log('[XHS Export] 调用底层 Pillow 原生像素转码引擎...');

const res = spawnSync('python', [scriptPath], { stdio: 'inherit' });
if (res.status !== 0) {
  process.exit(res.status || 1);
}
