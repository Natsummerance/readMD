"""Render the shared export AST as well-formed XHTML for EPUB readers."""
from html import escape
from . import parser


def inline(nodes):
    result = []
    for node in nodes:
        kind = node['t']
        if kind in ('text', 'code'):
            text = escape(node['v'])
            result.append('<code>' + text + '</code>' if kind == 'code' else text)
        elif kind in ('bold', 'italic', 'strike'):
            tag = {'bold': 'strong', 'italic': 'em', 'strike': 'del'}[kind]
            value = node['v']
            result.append('<%s>%s</%s>' % (tag, inline(value if isinstance(value, list) else parser.parse_inline(value)), tag))
        elif kind == 'link':
            url = node['href']
            if url.lower().startswith(('javascript:', 'data:', 'vbscript:')):
                result.append(inline(node['text']))
            else:
                result.append('<a href="%s">%s</a>' % (escape(url, quote=True), inline(node['text'])))
        elif kind == 'image':
            result.append('<img src="%s" alt="%s"/>' % (escape(node['src'], quote=True), escape(node['alt'], quote=True)))
        elif kind == 'math':
            result.append('<span class="math">%s</span>' % escape(node['latex']))
    return ''.join(result)


def render(blocks):
    result = []
    for block in blocks:
        kind = block['type']
        if kind in ('paragraph', 'heading'):
            tag = 'p' if kind == 'paragraph' else 'h%d' % block['level']
            result.append('<%s>%s</%s>' % (tag, inline(block['text']), tag))
        elif kind == 'table':
            head = '<tr>' + ''.join('<th>' + inline(cell) + '</th>' for cell in block['header']) + '</tr>'
            rows = ''.join('<tr>' + ''.join('<td>' + inline(cell) + '</td>' for cell in row) + '</tr>' for row in block['rows'])
            result.append('<table><thead>' + head + '</thead><tbody>' + rows + '</tbody></table>')
        elif kind == 'list':
            tag = 'ol' if block['items'] and block['items'][0].get('ordered') else 'ul'
            result.append('<%s>%s</%s>' % (tag, ''.join('<li>' + inline(item['text']) + '</li>' for item in block['items']), tag))
        elif kind == 'quote':
            result.append('<blockquote>' + render(block['blocks']) + '</blockquote>')
        elif kind == 'code':
            result.append('<pre><code>' + escape(block['content']) + '</code></pre>')
        elif kind == 'math':
            result.append('<p class="math">' + escape(block['latex']) + '</p>')
        elif kind == 'hr':
            result.append('<hr/>')
        elif kind == 'pagebreak':
            result.append('<div style="break-after:page"></div>')
    return '\n'.join(result)
