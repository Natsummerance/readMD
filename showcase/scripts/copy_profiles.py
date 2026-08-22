#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Evidence-aligned copy contracts for each selectable release mechanism."""
from __future__ import annotations

from typing import Any


PRESENTATION_FRAMES = {
    "outcome-led": [
        (
            "core",
            "文档已经写完，讲的时候还要复制进 PPT。这次把这一步砍掉：Markdown 直接放映。",
            "你会先拿哪一份 Markdown 试放映？评论区说说场景，我会把高频路径排进下一轮打磨。",
        ),
        (
            "workflow",
            "Markdown 写完只是前半段；这次不用再把它搬进 PPT，放映和修改在同一条工作流里。",
            "你会先用课程讲义、组会报告还是技术分享来试？评论区说说场景。",
        ),
        (
            "decision",
            "定稿后还要把 Markdown 复制成 PPT，这一步最磨人；现在不用复制，MD 可以直接上台。",
            "哪一份 Markdown 最适合先试？代码、表格还是公式？评论区告诉我。",
        ),
        (
            "source",
            "写完的 Markdown 不用复制到别的工具；ReadMD 把阅读、修改和放映接成一条路。",
            "你会拿哪类内容先跑一遍完整流程？讲义、论文还是技术笔记？",
        ),
    ],
    "identity-led": [
        (
            "core",
            "如果你要把笔记变成课程讲义、组会报告或论文汇报，就知道重做 PPT 有多烦。ReadMD 把这一步砍掉：Markdown 直接放映。",
            "你下一份要上台的 Markdown 是讲义、组会报告还是论文？评论区说说场景。",
        ),
        (
            "workflow",
            "如果你常写论文或组会报告，就不用再把 Markdown 复制进 PPT；同一份文件可以直接讲。",
            "你的下一场分享是课程、组会还是论文答辩？评论区对号入座。",
        ),
        (
            "decision",
            "要上台讲自己文档的人，最怕格式在复制时走样；MD 这次能直接保留工作流。",
            "你会讲哪一份材料？课程讲义、组会报告还是论文教程？",
        ),
        (
            "source",
            "给要把笔记变成正式汇报的人：不用重建 PPT，Markdown 就是演示入口。",
            "你会先讲哪一类文件？论文、组会记录还是课程讲义？",
        ),
    ],
    "mechanism-curiosity": [
        (
            "core",
            "很多人把 Markdown 写完就停在笔记里；其实同一份文件可以直接上台放映。ReadMD 让写作和演示留在同一条路径。",
            "你想先试哪类内容：代码、表格还是公式？评论区告诉我，我会优先打磨这条路径。",
        ),
        (
            "workflow",
            "写完的 Markdown 为什么能直接放映？因为写作、预览和演示被接成同一条路径。",
            "你想先验证哪一段链路：代码、表格还是公式？评论区选一个。",
        ),
        (
            "decision",
            "它不用把 Markdown 另存为幻灯片；写完后的阅读、修改和放映共用同一个源文件。",
            "你最想保住哪种排版？代码块、表格还是公式？评论区补充场景。",
        ),
        (
            "source",
            "从写作到上台只有一条路径；Markdown 不用导出成 PPT，显示状态由同一份源文件驱动。",
            "你会先用哪个机制试一遍：公式渲染、表格分片还是代码运行？",
        ),
    ],
}


def _generated_frames(profile: dict[str, Any]) -> dict[str, list[tuple[str, str, str]]]:
    artifact = profile["artifact"]
    action = profile["short_action"]
    task_hook = profile["task_hook"]
    return {
        "outcome-led": [
            ("core", f"{profile['opening']} Markdown 保持为主文件。", profile["cta"]),
            (
                "workflow",
                f"处理{task_hook}时，不用把它搬去别的工具：继续{action}，{profile['workflow']}。",
                f"你会先用{profile['options']}中的哪一类来试？评论区说说场景。",
            ),
            (
                "decision",
                f"处理{task_hook}时，{profile['decision_pain']}，这一步最磨人；现在不用绕路，Markdown 可以直接{action}。",
                f"哪份材料最适合先用来{action}？{profile['options']}都可以，评论区告诉我。",
            ),
            (
                "source",
                f"{artifact}不用脱离源文件；ReadMD 把处理{task_hook}、阅读、修改和{action}接成一条 Markdown 路。",
                f"你会先用哪类内容跑一遍完整流程？{profile['scenarios']}都可以。",
            ),
        ],
        "identity-led": [
            (
                "core",
                f"如果你要{profile['audience_task']}，就知道{profile['pain']}。ReadMD 把这一步砍掉：Markdown 可以直接{action}。",
                f"你下一份要处理{artifact}的材料是什么？{profile['scenarios']}都可以说说。",
            ),
            (
                "workflow",
                f"如果你常处理{task_hook}，就不用把它搬出 Markdown；继续{action}时，同一份文件能继续维护。",
                f"你下一场要处理{artifact}的场景是课程、组会还是个人项目？评论区对号入座。",
            ),
            (
                "decision",
                f"要{profile['audience_task']}的人，最怕处理{task_hook}时{profile['decision_pain']}；ReadMD 这次让 Markdown 能直接{action}。",
                f"你会先处理哪份材料？课程、报告还是{profile['options']}？",
            ),
            (
                "source",
                f"给要处理{profile['task_hook']}的人：不用换工具，Markdown 就是工作入口。",
                f"你会先放进哪一类文件？{profile['scenarios']}都可以。",
            ),
        ],
        "mechanism-curiosity": [
            (
                "core",
                f"很多人把{task_hook}停在旧流程里；其实它能留在 Markdown 里，直接{action}。ReadMD 让内容和结果走同一条路径。",
                f"你想先试哪类{artifact}：{profile['options']}？评论区告诉我，我会优先打磨这条路径。",
            ),
            (
                "workflow",
                f"{task_hook}为什么能稳定更新？因为它不用离开当前文档；{profile['workflow']}。",
                f"你想先验证哪段链路：{profile['options']}？评论区选一个。",
            ),
            (
                "decision",
                f"它不用一次性截图；{task_hook}的 Markdown 状态由源文件驱动。{profile['proof']}。",
                "你最想保住哪种状态：源文件、渲染结果还是可更新结构？评论区补充场景。",
            ),
            (
                "source",
                f"不用导出中间稿：从内容到{artifact}只有一条路径；{action}之后，Markdown 显示状态仍由源文件驱动。",
                f"你会先用哪份{artifact}试这条路径？评论区说说场景。",
            ),
        ],
    }


PROFILES: dict[str, dict[str, Any]] = {
    "overview.editor": {
        "artifact": "Markdown 稿件",
        "narrative_angle": "ReadMD 让写作和预览留在同一条工作流",
        "task_hook": "同屏改稿窗口",
        "short_action": "在同屏编辑器里改稿",
        "pain": "改一段 Markdown 就要切窗口核对格式，思路反复被打断",
        "decision_pain": "源稿改动后预览跟不上",
        "workflow": "源稿、光标位置和实时预览留在同一条工作流里",
        "options": "课程讲义、技术笔记、项目说明",
        "proof": "编辑器内容和预览状态由同一份源文件驱动",
        "audience_task": "反复打磨一份 Markdown 稿件",
        "scenarios": "课程讲义、技术笔记、项目说明",
        "titles": {
            "#36": "不用切窗口，Markdown同屏改",
            "#9": "Markdown改完，立刻看到排版",
            "#22": "给反复改稿的人做的MD工具",
            "#61": "别再把写作和预览拆开",
            "#12": "看完这{number}张，你会重新看MD编辑",
        },
        "opening": "改一段就要切窗口核对格式。这次把这一步砍掉：Markdown 源稿和实时预览在同一屏。",
        "primary_paragraph": "编辑器和实时预览在同一屏里，先改内容再确认排版，不用在几个窗口之间来回追版本。",
        "saved_step": "在几个窗口之间追版本",
        "cover": {
            "formula_id": "#36",
            "title": "同屏改稿",
            "caption": "改完立刻看到排版，不用切窗口。",
        },
        "cta": "你会先用哪份 Markdown 同屏修改？课程讲义、技术笔记还是项目说明？",
        "hook_contract": {
            "task": ["改", "窗口", "格式"],
            "removal": ["不用", "砍掉", "直接"],
            "mechanism": ["Markdown", "同屏"],
        },
    },
    "presentation.reveal": {
        "artifact": "Markdown 演示稿",
        "narrative_angle": "ReadMD 让同一份 Markdown 从阅读、编辑直接走到上台放映",
        "task_hook": "上台放映的演示稿",
        "short_action": "直接放映",
        "pain": "文档已经写完，讲的时候还要复制进 PPT",
        "decision_pain": "定稿后还要把 Markdown 复制成 PPT",
        "workflow": "写作、预览和放映被接成同一条工作流",
        "options": "代码、表格、公式",
        "proof": "阅读、修改和放映共用同一个源文件",
        "audience_task": "把笔记变成课程讲义、组会报告或论文汇报",
        "scenarios": "课程讲义、组会报告、论文汇报",
        "titles": {
            "#36": "不用重做PPT，Markdown直接放映",
            "#9": "Markdown写完，居然能直接上台",
            "#22": "给要上台讲文档的人做的MD工具",
            "#61": "别再把Markdown只当笔记了",
            "#12": "看完这{number}张，你会重新看Markdown",
        },
        "opening": "文档已经写完，讲的时候还要复制进 PPT。这次把这一步砍掉：Markdown 直接放映。",
        "primary_paragraph": "放映界面可以直接换主题、调字号、切开场和转场；AST 保护分片会尽量保住代码块、表格和公式，不让长文档在幻灯片里被腰斩。",
        "saved_step": "重新做一遍演示稿",
        "cover": {
            "formula_id": "#36",
            "title": "写完就能讲",
            "caption": "Markdown 直接放映，不用重做 PPT。",
        },
        "cta": "你会先拿哪一份 Markdown 试放映？评论区说说场景，我会把高频路径排进下一轮打磨。",
        "hook_contract": {
            "task": ["写完", "复制", "PPT", "讲"],
            "removal": ["砍掉", "不用", "直接"],
            "mechanism": ["Markdown", "MD", "放映"],
        },
        "frames": PRESENTATION_FRAMES,
    },
    "editor.diagram-picker": {
        "artifact": "科研图表",
        "narrative_angle": "ReadMD 把科研图表放进同一条 Markdown 工作流",
        "task_hook": "科研图表语法",
        "short_action": "从面板选图",
        "pain": "画科研图表还要回忆语法，改一次就很折磨",
        "decision_pain": "图表语法一改就废",
        "workflow": "图表结构、源码和文档内容在同一条 Markdown 工作流里",
        "options": "流程图、架构图、时序图",
        "proof": "面板结构、源码和渲染结果共用同一份文档",
        "audience_task": "把论文或技术笔记里的关系画成科研图表",
        "scenarios": "论文、实验记录、技术报告",
        "titles": {
            "#36": "不用记语法，科研图表直接选",
            "#9": "科研图表，居然可以面板里选",
            "#22": "给论文画图的人做的MD工具",
            "#61": "别再手写一版就废的图表语法",
            "#12": "看完这{number}张，你会重新看科研图",
        },
        "opening": "画科研图表还要回忆语法，改一次就很折磨。这次不用硬记：Markdown 面板选择结构，结果留在文档里。",
        "primary_paragraph": "图表从面板里选，渲染结果留在文档里；适合论文、报告和需要长期维护的技术笔记。",
        "saved_step": "手写一遍就报废的图表语法",
        "cover": {
            "formula_id": "#36",
            "title": "图表直接选",
            "caption": "科研图从面板进入 Markdown，不背语法。",
        },
        "cta": "你会先给论文里的哪类内容画图？流程、架构还是时序？评论区说说场景。",
        "hook_contract": {
            "task": ["图表", "语法", "画"],
            "removal": ["不用", "面板", "选择", "直接"],
            "mechanism": ["科研图表", "文档", "Markdown"],
        },
    },
    "academic.latex-bib": {
        "artifact": "学术排版",
        "narrative_angle": "ReadMD 让公式、定理和引用共用一条学术排版路径",
        "task_hook": "公式和文献格式",
        "short_action": "在文档里排公式",
        "pain": "公式和文献格式总在交稿前折磨人",
        "decision_pain": "公式换个工具就走样",
        "workflow": "公式、定理盒子和引用沿用同一套 Markdown 排版规则",
        "options": "推导、定理、参考文献",
        "proof": "公式、定理盒子和引用由同一份源文件驱动",
        "audience_task": "把论文和学术笔记排到能交的程度",
        "scenarios": "课程讲义、论文推导、学术笔记",
        "titles": {
            "#36": "不用换模板，公式保持论文排版",
            "#9": "LaTeX公式，居然不用来回调",
            "#22": "给写论文的人做的学术MD工具",
            "#61": "别再为公式格式重复返工",
            "#12": "看完这{number}张，你会重新看学术排版",
        },
        "opening": "公式和文献格式总在交稿前折磨人。这次不用另起工具：LaTeX 和引用留在 Markdown 里。",
        "primary_paragraph": "公式、定理盒子和参考文献沿用同一套排版；写作时不用在笔记、LaTeX 和最终稿之间反复搬运。",
        "saved_step": "在另一个工具里重调公式格式",
        "cover": {
            "formula_id": "#36",
            "title": "公式不走样",
            "caption": "LaTeX 和引用留在同一条工作流。",
        },
        "cta": "你下一份要处理的是课程讲义还是论文？评论区说说公式场景。",
        "hook_contract": {
            "task": ["公式", "文献", "交稿"],
            "removal": ["不用", "留在", "直接"],
            "mechanism": ["LaTeX", "Markdown", "排版"],
        },
    },
    "editor.code-chunk": {
        "artifact": "文档代码块",
        "narrative_angle": "ReadMD 让文档里的代码就地运行并保留输出",
        "task_hook": "可运行代码块",
        "short_action": "就地运行代码",
        "pain": "教程写到代码就要切出去验证，上下文很容易断",
        "decision_pain": "示例代码和文档说明脱节",
        "workflow": "代码、运行状态和输出留在同一条文档路径里",
        "options": "配置脚本、算法示例、数据处理",
        "proof": "代码块、运行按钮和输出区共用同一份文档",
        "audience_task": "写出能跟着复现的技术教程",
        "scenarios": "代码教程、技术笔记、示例文档",
        "titles": {
            "#36": "不用切终端，文档代码直接跑",
            "#9": "Markdown代码块，居然能运行",
            "#22": "给写技术教程的人做的MD工具",
            "#61": "别再让示例代码停在文档里",
            "#12": "看完这{number}张，你会重新看代码块",
        },
        "opening": "教程写到代码，还要切出去验证一遍。这次不用切换：代码块直接在 Markdown 里运行。",
        "primary_paragraph": "代码块保留运行按钮、状态和输出；读者看到的不是死代码，而是能跟着复现的步骤。",
        "saved_step": "另开终端重复验证示例",
        "cover": {
            "formula_id": "#36",
            "title": "代码就地跑",
            "caption": "教程里的示例能运行，读者能复现。",
        },
        "cta": "你会先验证哪段代码？配置脚本、算法示例还是数据处理？评论区挑一个。",
        "hook_contract": {
            "task": ["教程", "代码", "验证"],
            "removal": ["不用", "直接", "就地"],
            "mechanism": ["Markdown", "代码块", "运行"],
        },
    },
    "convert.home": {
        "artifact": "零散资料",
        "narrative_angle": "ReadMD 把网页、PDF 和 Word 收进同一条本地工作流",
        "task_hook": "网页、PDF 和 Word 资料",
        "short_action": "收进本地工作台",
        "pain": "网页、Word 和 PDF 分散在不同窗口，资料格式很难归拢",
        "decision_pain": "资料转换后找不到来源",
        "workflow": "打开、转换、AI 和抓取入口接在同一条本地工作流里",
        "options": "网页、PDF、Word",
        "proof": "转换入口、新标签页和源文件都在本地工作台里",
        "audience_task": "把零散资料整理成可检索的 Markdown",
        "scenarios": "网页剪藏、PDF 资料、Word 文档",
        "titles": {
            "#36": "不用来回搬，资料收进同一个入口",
            "#9": "零散资料，居然能进一个工作台",
            "#22": "给整理资料的人做的MD工具",
            "#61": "别再让资料散落在临时工具里",
            "#12": "看完这{number}张，你会重新看资料整理",
        },
        "opening": "网页、Word 和 PDF 分散在不同窗口，资料格式很难归拢。这次不用搬运：它们进入同一条 Markdown 工作流。",
        "primary_paragraph": "打开、转换、AI 和网页抓取都从一个本地入口开始，资料不会被拆到一串临时工具里。",
        "saved_step": "在不同工具之间搬运资料",
        "cover": {
            "formula_id": "#36",
            "title": "资料进工作台",
            "caption": "网页、PDF 和 Word 收进同一个入口。",
        },
        "cta": "你会先把网页、PDF 还是 Word 收进来？评论区说说整理场景。",
        "hook_contract": {
            "task": ["网页", "Word", "PDF", "资料"],
            "removal": ["不用", "进入", "收进"],
            "mechanism": ["Markdown", "本地", "工作流"],
        },
    },
    "sharing.export": {
        "artifact": "本地文档",
        "narrative_angle": "ReadMD 让本地文档直接生成可控制的共享入口",
        "task_hook": "分享或导出的本地文档",
        "short_action": "生成共享入口",
        "pain": "想把 Markdown 发给别人，还要先导出副本，版本很快对不上",
        "decision_pain": "分享出去的文件已经过期",
        "workflow": "共享入口、访问令牌和当前文档留在同一条本地工作流里",
        "options": "讲义、报告、长笔记",
        "proof": "扫码入口、随机令牌和启停控制都指向当前文档",
        "audience_task": "把正在维护的 Markdown 发给别人看",
        "scenarios": "讲义、报告、长笔记",
        "titles": {
            "#36": "不用重复导出，文档直接分享",
            "#9": "本地文档，居然能直接到手机",
            "#22": "给要分享文档的人做的MD工具",
            "#61": "别再为了分享多存一份副本",
            "#12": "看完这{number}张，你会重新看文档分享",
        },
        "opening": "想把 Markdown 发给别人，还要先导出一份副本，版本很快对不上。这次不用多存：当前文档直接生成共享入口。",
        "primary_paragraph": "局域网共享面板提供扫码入口、随机令牌和启停控制；手机打开的是当前文档，不需要对方装软件。",
        "saved_step": "为了分享再导出一份副本",
        "cover": {
            "formula_id": "#36",
            "title": "文档到手机",
            "caption": "本地文档生成共享入口，不用再导出副本。",
        },
        "cta": "你会先分享哪份 Markdown？讲义、报告还是长笔记？",
        "hook_contract": {
            "task": ["分享", "导出", "版本"],
            "removal": ["不用", "直接", "当前"],
            "mechanism": ["Markdown", "共享入口"],
        },
    },
}


def profile_for_story(story: dict[str, Any]) -> dict[str, Any]:
    return PROFILES.get(str(story.get("primary_shot", "overview.editor")), PROFILES["overview.editor"])


def frames_for_story(story: dict[str, Any]) -> dict[str, list[tuple[str, str, str]]]:
    profile = profile_for_story(story)
    return profile.get("frames") or _generated_frames(profile)
