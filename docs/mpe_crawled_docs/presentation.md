# 幻灯片制作 (Reveal.js)

将 Markdown 快速编译为技术演说 PPT：

## 分页符
- `<!-- slide -->`: 横向下一页
- `<!-- slide -->`: 垂直下钻页

## Front-matter 配置
```yaml
---
presentation:
  theme: league.css
  transition: slide
  slideNumber: true
  enableSpeakerNotes: true
---
```

## 演讲者视图 (Speaker Notes)
在任意页面添加 `<!-- slide -->`，放映时按 `S` 键唤起独立演讲者计时窗口。
