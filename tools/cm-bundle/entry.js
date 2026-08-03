import { EditorState, Compartment } from '@codemirror/state';
import { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection, highlightActiveLineGutter, dropCursor, rectangularSelection } from '@codemirror/view';
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands';
import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { languages } from '@codemirror/language-data';
import { autocompletion, closeBrackets, closeBracketsKeymap, completionKeymap, snippet } from '@codemirror/autocomplete';
import { oneDark } from '@codemirror/theme-one-dark';
import { syntaxHighlighting, defaultHighlightStyle, bracketMatching, indentOnInput, foldGutter } from '@codemirror/language';

window.ReadMDCodeMirror = {
  EditorState, Compartment, EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection, highlightActiveLineGutter, dropCursor, rectangularSelection,
  defaultKeymap, history, historyKeymap, indentWithTab,
  markdown, markdownLanguage, languages,
  autocompletion, closeBrackets, closeBracketsKeymap, completionKeymap, snippet,
  oneDark, syntaxHighlighting, defaultHighlightStyle, bracketMatching, indentOnInput, foldGutter,
};
