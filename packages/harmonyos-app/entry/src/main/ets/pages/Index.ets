/**
 * ReadMD HarmonyOS 主界面 (ArkUI + ArkWeb 容器)
 */

import webview from '@ohos.web.webview';
import { ReadMDBridge } from '../bridge/ReadMDBridge';

@Entry
@Component
struct Index {
  private controller: webview.WebviewController = new webview.WebviewController();
  private bridge: ReadMDBridge = new ReadMDBridge(getContext(this));

  build() {
    Column() {
      Web({ src: $rawfile('index.html'), controller: this.controller })
        .javaScriptAccess(true)
        .domStorageAccess(true)
        .fileAccess(true)
        .imageAccess(true)
        .onlineImageAccess(true)
        .javaScriptProxy({
          object: this.bridge,
          name: 'py',
          methodList: [
            'get_system_language',
            'read_clipboard',
            'choose_any_file',
            'choose_many_files',
            'save_file',
            'save_as',
            'show_toast'
          ],
          controller: this.controller
        })
        .width('100%')
        .height('100%')
    }
    .width('100%')
    .height('100%')
  }
}
