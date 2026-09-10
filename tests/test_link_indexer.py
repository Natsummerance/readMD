# -*- coding: utf-8 -*-
"""Tests for LinkIndexer: bi-directional link indexing, wikilinks, backlinks, deadlinks, and graph generation."""

import os
import shutil
import sys
import tempfile
import time
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules import link_indexer


class TestLinkIndexer(unittest.TestCase):
    """Test link extraction, resolving, database persistence, backlinks, and graph data."""

    def setUp(self):
        self.test_dir = tempfile.mkdtemp(prefix="readmd_link_test_")
        self.db_path = os.path.join(self.test_dir, ".readmd", "index.db")
        self.indexer = link_indexer.LinkIndexer(db_path=self.db_path)

    def tearDown(self):
        self.indexer.close()
        shutil.rmtree(self.test_dir, ignore_errors=True)

    def test_workspace_scan_preserves_sibling_prefix_and_literal_wildcards(self):
        for name in ('notes', 'notes-extra', 'notes_1', 'notesX1'):
            directory = os.path.join(self.test_dir, name)
            os.makedirs(directory)
            with open(os.path.join(directory, 'a.md'), 'w', encoding='utf-8') as f:
                f.write('[[missing]]')
            self.indexer.index_directory(directory)
        for name in ('notes', 'notes_1'):
            directory = os.path.join(self.test_dir, name)
            result = self.indexer.index_directory(directory)
            self.assertEqual(result['deleted_count'], 0)
            self.assertEqual(len(self.indexer.get_deadlinks(directory)), 1)
            self.assertEqual(len([n for n in self.indexer.get_graph_data(directory)['nodes']
                                  if not n['is_deadlink']]), 1)
        self.assertEqual(len(self.indexer.get_deadlinks()), 4)

    def test_deleted_target_becomes_deadlink_without_editing_source(self):
        source = os.path.join(self.test_dir, 'source.md')
        target = os.path.join(self.test_dir, 'target.md')
        with open(source, 'w', encoding='utf-8') as f:
            f.write('[[target]]')
        with open(target, 'w', encoding='utf-8') as f:
            f.write('# Target')
        self.indexer.index_directory(self.test_dir)
        self.assertEqual(self.indexer.get_forward_links(source)[0]['target_path'], target)
        os.remove(target)
        self.indexer.index_directory(self.test_dir)
        self.assertIsNone(self.indexer.get_forward_links(source)[0]['target_path'])
        self.assertEqual(len(self.indexer.get_deadlinks(self.test_dir)), 1)

    def test_extract_links_wikilinks_and_markdown(self):
        """Test extraction of various [[wikilink]] formats and markdown links."""
        sample_md = (
            "# Main Doc\n"
            "This links to [[PageA]] and [[PageB|Custom Alias]].\n"
            "Also check [[PageC#Heading1]] and [[PageD#Heading2|Alias D]].\n"
            "Here is standard [Markdown Doc](notes/sub.md).\n"
            "External [Web](https://example.com) and anchor [Top](#top) should be ignored.\n"
        )
        links = self.indexer.extract_links(sample_md)
        # Expected 5 internal links: PageA, PageB, PageC, PageD, notes/sub.md
        self.assertEqual(len(links), 5)

        # [[PageA]]
        l0 = links[0]
        self.assertEqual(l0['target_raw'], 'PageA')
        self.assertEqual(l0['target_clean'], 'PageA')
        self.assertIsNone(l0['alias'])
        self.assertIsNone(l0['heading'])
        self.assertTrue(l0['is_wikilink'])
        self.assertEqual(l0['line_no'], 2)

        # [[PageB|Custom Alias]]
        l1 = links[1]
        self.assertEqual(l1['target_clean'], 'PageB')
        self.assertEqual(l1['alias'], 'Custom Alias')
        self.assertIsNone(l1['heading'])
        self.assertTrue(l1['is_wikilink'])

        # [[PageC#Heading1]]
        l2 = links[2]
        self.assertEqual(l2['target_clean'], 'PageC')
        self.assertEqual(l2['heading'], 'Heading1')
        self.assertIsNone(l2['alias'])
        self.assertTrue(l2['is_wikilink'])

        # [[PageD#Heading2|Alias D]]
        l3 = links[3]
        self.assertEqual(l3['target_clean'], 'PageD')
        self.assertEqual(l3['heading'], 'Heading2')
        self.assertEqual(l3['alias'], 'Alias D')
        self.assertTrue(l3['is_wikilink'])

        # [Markdown Doc](notes/sub.md)
        l4 = links[4]
        self.assertEqual(l4['target_clean'], 'notes/sub.md')
        self.assertEqual(l4['alias'], 'Markdown Doc')
        self.assertFalse(l4['is_wikilink'])

    def test_extract_links_skips_code_blocks(self):
        """Ensure links inside fenced code blocks and inline code are masked and not extracted."""
        sample_md = (
            "Normal [[RealLink]].\n"
            "```python\n"
            "# Code block with [[FakeLinkInCode]] and [FakeMD](fake.md)\n"
            "```\n"
            "Inline code `[[FakeInline]]` should be ignored.\n"
            "Another real link [RealDoc](real.md).\n"
        )
        links = self.indexer.extract_links(sample_md)
        targets = [l['target_clean'] for l in links]
        self.assertIn('RealLink', targets)
        self.assertIn('real.md', targets)
        self.assertNotIn('FakeLinkInCode', targets)
        self.assertNotIn('fake.md', targets)
        self.assertNotIn('FakeInline', targets)
        self.assertEqual(len(links), 2)

    def test_index_directory_and_incremental_scan(self):
        """Test scanning a folder, building links, and skipping unchanged files on incremental scan."""
        # Create doc1.md and doc2.md
        doc1_path = os.path.join(self.test_dir, "doc1.md")
        doc2_path = os.path.join(self.test_dir, "doc2.md")

        with open(doc1_path, "w", encoding="utf-8") as f:
            f.write("# Doc 1\nLinks to [[doc2]] and [[MissingNote]].")

        with open(doc2_path, "w", encoding="utf-8") as f:
            f.write("# Doc 2\nNo links here.")

        # First scan
        res1 = self.indexer.index_directory(self.test_dir)
        self.assertEqual(res1['scanned_count'], 2)
        self.assertEqual(res1['indexed_count'], 2)

        # Forward links of doc1
        fwd = self.indexer.get_forward_links(doc1_path)
        self.assertEqual(len(fwd), 2)

        # Backlinks of doc2
        bwd = self.indexer.get_backlinks(doc2_path)
        self.assertEqual(len(bwd), 1)
        self.assertEqual(os.path.normpath(bwd[0]['source_path']), os.path.normpath(doc1_path))

        # Dead links check
        dead = self.indexer.get_deadlinks(self.test_dir)
        self.assertEqual(len(dead), 1)
        self.assertEqual(dead[0]['target_clean'], 'MissingNote')

        # Incremental scan without modifying anything
        res2 = self.indexer.index_directory(self.test_dir)
        self.assertEqual(res2['scanned_count'], 2)
        self.assertEqual(res2['indexed_count'], 0)  # 0 modified, all skipped!

    def test_graph_data_export(self):
        """Test formatting graph data with nodes, edges, degrees, and deadlink status."""
        docA = os.path.join(self.test_dir, "A.md")
        docB = os.path.join(self.test_dir, "B.md")
        docC = os.path.join(self.test_dir, "C.md")

        with open(docA, "w", encoding="utf-8") as f:
            f.write("# Note A\n[[B]] and [[C]] and [[NonExistent]]")
        with open(docB, "w", encoding="utf-8") as f:
            f.write("# Note B\n[[C]]")
        with open(docC, "w", encoding="utf-8") as f:
            f.write("# Note C\nBack to [[A]]")

        self.indexer.index_directory(self.test_dir)
        graph = self.indexer.get_graph_data(self.test_dir)

        self.assertIn('nodes', graph)
        self.assertIn('edges', graph)
        self.assertIn('stats', graph)

        node_ids = {n['id'] for n in graph['nodes']}
        self.assertIn('A.md', node_ids)
        self.assertIn('B.md', node_ids)
        self.assertIn('C.md', node_ids)
        self.assertIn('NonExistent', node_ids)

        # Find NonExistent node and verify deadlink flag
        dead_node = next(n for n in graph['nodes'] if n['id'] == 'NonExistent')
        self.assertTrue(dead_node['is_deadlink'])

        # Edge count: A->B, A->C, A->NonExistent, B->C, C->A = 5 edges
        self.assertEqual(len(graph['edges']), 5)

    def test_delete_file_cleanup(self):
        """Test that removing a file from disk removes it and its links on next index scan."""
        temp_doc = os.path.join(self.test_dir, "temp.md")
        with open(temp_doc, "w", encoding="utf-8") as f:
            f.write("# Temp Doc\n[[TargetNote]]")

        res = self.indexer.index_directory(self.test_dir)
        self.assertEqual(res['indexed_count'], 1)
        self.assertEqual(len(self.indexer.get_forward_links(temp_doc)), 1)

        # Delete file
        os.remove(temp_doc)
        res_del = self.indexer.index_directory(self.test_dir)
        self.assertEqual(res_del['deleted_count'], 1)
        self.assertEqual(len(self.indexer.get_forward_links(temp_doc)), 0)

    def test_internal_heading_link(self):
        """Test that internal heading links [[#Section]] resolve to the document itself."""
        doc = os.path.join(self.test_dir, "self_ref.md")
        with open(doc, "w", encoding="utf-8") as f:
            f.write("# Self Ref\nJump to [[#Section Intro]].")

        self.indexer.index_directory(self.test_dir)
        links = self.indexer.get_forward_links(doc)
        self.assertEqual(len(links), 1)
        self.assertEqual(links[0]['heading'], 'Section Intro')
        self.assertEqual(os.path.normpath(links[0]['target_path']), os.path.normpath(doc))

    def test_api_bridge_methods(self):
        """Test Api() bridge methods for link indexing and graph querying."""
        from readmd import Api
        api = Api()
        doc = os.path.join(self.test_dir, "bridge_doc.md")
        with open(doc, "w", encoding="utf-8") as f:
            f.write("# Bridge Doc\n[[TargetB]]")

        # Mock or patch get_indexer to use self.indexer
        from unittest.mock import patch
        with patch('src.readmd_modules.link_indexer.get_indexer', return_value=self.indexer):
            res_idx = api.index_directory_links(self.test_dir)
            self.assertTrue(res_idx.get('ok'))

            res_graph = api.get_links_graph(self.test_dir)
            self.assertTrue(res_graph.get('ok'))
            self.assertIn('graph', res_graph)
            self.assertGreater(len(res_graph['graph']['nodes']), 0)

            res_backlinks = api.get_backlinks(doc)
            self.assertTrue(res_backlinks.get('ok'))
            self.assertIn('forward_links', res_backlinks)
            self.assertEqual(len(res_backlinks['forward_links']), 1)


if __name__ == '__main__':
    unittest.main()
