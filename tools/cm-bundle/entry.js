import { EditorState, Compartment, EditorSelection, Prec, StateField, StateEffect, RangeSetBuilder, Transaction } from '@codemirror/state';
import { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection, highlightActiveLineGutter, dropCursor, rectangularSelection, crosshairCursor, Decoration, ViewPlugin, WidgetType, placeholder, highlightSpecialChars } from '@codemirror/view';
import { defaultKeymap, history, historyKeymap, indentWithTab, indentMore, indentLess, insertNewlineAndIndent, undo, redo } from '@codemirror/commands';
import { markdown, markdownLanguage, markdownKeymap, insertNewlineContinueMarkup, insertNewlineContinueMarkupCommand, deleteMarkupBackward, pasteURLAsLink } from '@codemirror/lang-markdown';
import { languages } from '@codemirror/language-data';
import { autocompletion, closeBrackets, closeBracketsKeymap, completionKeymap, snippet, startCompletion, closeCompletion, completionStatus } from '@codemirror/autocomplete';
import { oneDark } from '@codemirror/theme-one-dark';
import { syntaxHighlighting, defaultHighlightStyle, bracketMatching, indentOnInput, foldGutter, HighlightStyle, syntaxTree, indentUnit } from '@codemirror/language';
import { tags } from '@lezer/highlight';

window.ReadMDCodeMirror = {
  EditorState, Compartment, EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection, highlightActiveLineGutter, dropCursor, rectangularSelection,
  defaultKeymap, history, historyKeymap, indentWithTab,
  markdown, markdownLanguage, languages,
  autocompletion, closeBrackets, closeBracketsKeymap, completionKeymap, snippet,
  oneDark, syntaxHighlighting, defaultHighlightStyle, bracketMatching, indentOnInput, foldGutter,
  // ReadMD editor upgrade (additive; everything above stays for compatibility)
  EditorSelection, Prec, StateField, StateEffect, RangeSetBuilder, Transaction,
  crosshairCursor, Decoration, ViewPlugin, WidgetType, placeholder, highlightSpecialChars,
  indentMore, indentLess, insertNewlineAndIndent, undo, redo,
  markdownKeymap, insertNewlineContinueMarkup, insertNewlineContinueMarkupCommand, deleteMarkupBackward, pasteURLAsLink,
  startCompletion, closeCompletion, completionStatus,
  HighlightStyle, syntaxTree, indentUnit, tags,
};
