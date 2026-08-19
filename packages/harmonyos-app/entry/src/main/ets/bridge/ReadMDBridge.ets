/**
 * ReadMD HarmonyOS (ArkTS) 与 Webview (ArkWeb) 双向交互桥接器
 * 适配 HarmonyOS NEXT 纯血鸿蒙与 OpenHarmony 4.1/5.0
 */

import pasteboard from '@ohos.pasteboard';
import picker from '@ohos.file.picker';
import fs from '@ohos.file.fs';
import i18n from '@ohos.i18n';
import promptAction from '@ohos.promptAction';

export class ReadMDBridge {
  private context: Context;

  constructor(ctx: Context) {
    this.context = ctx;
  }

  /** 获取系统当前语言 */
  public get_system_language(): string {
    try {
      const sysLocale = i18n.System.getSystemLocale();
      if (sysLocale.startsWith('zh-Hant') || sysLocale.startsWith('zh-TW') || sysLocale.startsWith('zh-HK')) {
        return sysLocale.includes('HK') ? 'zh-HK' : 'zh-TW';
      }
      if (sysLocale.startsWith('zh')) return 'zh-CN';
      return sysLocale.split('-')[0] || 'en';
    } catch (e) {
      return 'zh-CN';
    }
  }

  /** 读取鸿蒙系统剪贴板 */
  public async read_clipboard(token?: string): Promise<Record<string, Object>> {
    try {
      const sysPasteboard = pasteboard.getSystemPasteboard();
      const pasteData = await sysPasteboard.getData();
      if (pasteData.hasType(pasteboard.MIMETYPE_TEXT_PLAIN)) {
        const text = pasteData.getPrimaryText();
        return { text: text, html: '', source_type: 'text' };
      }
      if (pasteData.hasType(pasteboard.MIMETYPE_TEXT_HTML)) {
        const html = pasteData.getPrimaryHtml();
        return { text: '', html: html, source_type: 'html' };
      }
    } catch (e) {
      promptAction.showToast({ message: '读取剪贴板失败' });
    }
    return { text: '', html: '', source_type: 'empty' };
  }

  /** 弹出系统文件选择器（单选） */
  public async choose_any_file(): Promise<string> {
    try {
      const docSelectOptions = new picker.DocumentSelectOptions();
      docSelectOptions.maxSelectNumber = 1;
      const docViewPicker = new picker.DocumentViewPicker(this.context);
      const results = await docViewPicker.select(docSelectOptions);
      return results && results.length > 0 ? results[0] : '';
    } catch (e) {
      return '';
    }
  }

  /** 弹出系统文件选择器（多选，支持 OCR 与多文件导入） */
  public async choose_many_files(): Promise<string[]> {
    try {
      const docSelectOptions = new picker.DocumentSelectOptions();
      docSelectOptions.maxSelectNumber = 50;
      const docViewPicker = new picker.DocumentViewPicker(this.context);
      const results = await docViewPicker.select(docSelectOptions);
      return results || [];
    } catch (e) {
      return [];
    }
  }

  /** 保存文件 */
  public async save_file(path: string, content: string, encoding: string): Promise<Record<string, Object>> {
    try {
      const file = fs.openSync(path, fs.OpenMode.READ_WRITE | fs.OpenMode.CREATE | fs.OpenMode.TRUNC);
      fs.writeSync(file.fd, content);
      fs.closeSync(file);
      return { ok: true };
    } catch (e) {
      return { ok: false, error: String(e) };
    }
  }

  /** 另存为文件 */
  public async save_as(content: string, suggestedName: string): Promise<string> {
    try {
      const docSaveOptions = new picker.DocumentSaveOptions();
      docSaveOptions.newFileNames = [suggestedName];
      const docViewPicker = new picker.DocumentViewPicker(this.context);
      const results = await docViewPicker.save(docSaveOptions);
      if (results && results.length > 0) {
        const targetUri = results[0];
        const file = fs.openSync(targetUri, fs.OpenMode.READ_WRITE | fs.OpenMode.CREATE | fs.OpenMode.TRUNC);
        fs.writeSync(file.fd, content);
        fs.closeSync(file);
        return targetUri;
      }
    } catch (e) {
      promptAction.showToast({ message: '保存失败: ' + String(e) });
    }
    return '';
  }

  /** 显示系统 Toast 提示 */
  public show_toast(msg: string): void {
    promptAction.showToast({ message: msg, duration: 2500 });
  }
}
