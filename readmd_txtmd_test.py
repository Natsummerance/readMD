# -*- coding: utf-8 -*-
import os, sys, tempfile, unittest
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from readmd_modules import txtmd


class TestTxtMd(unittest.TestCase):

    def test_chapter_headings(self):
        text = '第一章 概述\n\n这是正文内容。\n\n第二章 方法\n\n正文。\n\n第三章 结论\n\n正文。\n'
        md, stats = txtmd.to_markdown(text)
        self.assertTrue(md.startswith('## 目录'))
        self.assertIn('# 第一章 概述', md)
        self.assertIn('# 第二章 方法', md)
        self.assertIn('- [第一章 概述](#第一章-概述)', md)
        self.assertEqual(stats['headings'], 3)
        self.assertTrue(stats['toc'])

    def test_cn_number_headings(self):
        text = '一、背景\n\n内容。\n\n二、目标\n\n内容。\n\n三、方案\n\n内容。\n'
        md, stats = txtmd.to_markdown(text)
        self.assertIn('## 一、背景', md)
        self.assertIn('## 二、目标', md)
        self.assertEqual(stats['headings'], 3)

    def test_numeric_sub_headings(self):
        text = '1. 引言\n\n1.1 动机\n\n1.2 贡献\n\n2. 方法\n\n正文。\n'
        md, stats = txtmd.to_markdown(text)
        self.assertIn('## 1. 引言', md)
        self.assertIn('### 1.1 动机', md)
        self.assertIn('### 1.2 贡献', md)
        self.assertEqual(stats['headings'], 4)

    def test_tab_table(self):
        text = '姓名\t年龄\t城市\n张三\t20\t北京\n李四\t30\t上海\n'
        md, stats = txtmd.to_markdown(text)
        self.assertIn('| 姓名 | 年龄 | 城市 |', md)
        self.assertIn('| --- | --- | --- |', md)
        self.assertIn('| 张三 | 20 | 北京 |', md)
        self.assertEqual(stats['tables'], 1)

    def test_space_aligned_table(self):
        text = '姓名    年龄    城市\n张三    20    北京\n李四    30    上海\n'
        md, stats = txtmd.to_markdown(text)
        self.assertIn('| 姓名 | 年龄 | 城市 |', md)
        self.assertIn('| --- | --- | --- |', md)
        self.assertEqual(stats['tables'], 1)

    def test_bullet_list(self):
        text = '要点：\n\n• 第一项\n• 第二项\n• 第三项\n'
        md, stats = txtmd.to_markdown(text)
        self.assertIn('- 第一项', md)
        self.assertIn('- 第二项', md)
        self.assertEqual(stats['lists'], 3)

    def test_cn_numbered_list(self):
        text = '步骤：\n\n1、初始化\n2、编译\n3、运行\n'
        md, stats = txtmd.to_markdown(text)
        self.assertIn('1. 初始化', md)
        self.assertIn('1. 编译', md)
        self.assertEqual(stats['lists'], 3)

    def test_plain_text_unchanged(self):
        text = '这是一个普通的段落，没有任何结构特征，只是普通的一句话，\n它跨了多行继续写下去。\n'
        md, stats = txtmd.to_markdown(text)
        self.assertEqual(md, text)
        self.assertFalse(stats['changed'])

    def test_duplicate_slug_dedup(self):
        text = '一、概述\n\n内容。\n\n一、概述\n\n内容。\n\n一、概述\n\n内容。\n'
        md, stats = txtmd.to_markdown(text)
        self.assertIn('(#一概述)', md)
        self.assertIn('(#一概述-1)', md)
        self.assertIn('(#一概述-2)', md)

    def test_fence_content_untouched(self):
        text = '```\n1. 代码里的列表\n2. 不应转换\n```\n'
        md, stats = txtmd.to_markdown(text)
        self.assertIn('1. 代码里的列表', md)
        self.assertEqual(stats['changed'], False)

    def test_long_line_not_heading(self):
        text = '一、这是一个非常非常非常非常非常非常非常非常非常非常非常长的开头句子，它不是标题而是正文段落的一部分，会超过长度上限。\n'
        md, stats = txtmd.to_markdown(text)
        self.assertNotIn('##', md)
        self.assertFalse(stats['changed'])

    def test_gb18030_encoding(self):
        with tempfile.NamedTemporaryFile('wb', suffix='.txt', delete=False) as f:
            f.write('一、中文标题\n\n正文内容。\n'.encode('gb18030'))
            path = f.name
        try:
            text, enc = txtmd.read_text(path)
            self.assertEqual(enc, 'gb18030')
            self.assertIn('一、中文标题', text)
        finally:
            os.unlink(path)

    def test_short_line_heading(self):
        text = '项目简介\n\n这是一段正文，长度足够长，不会被误认为标题，因为它包含完整的句子结构。\n\n实现细节\n\n另一段正文。\n\n测试结果\n\n又一段正文。\n'
        md, stats = txtmd.to_markdown(text)
        self.assertIn('## 项目简介', md)
        self.assertIn('## 实现细节', md)
        self.assertIn('## 测试结果', md)
        self.assertEqual(stats['headings'], 3)


if __name__ == '__main__':
    unittest.main(verbosity=2)