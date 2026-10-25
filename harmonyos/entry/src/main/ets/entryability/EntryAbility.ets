import { UIAbility } from '@kit.AbilityKit';
import { window, KeyboardAvoidMode } from '@kit.ArkUI';
import { BusinessError } from '@kit.BasicServicesKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { fileIo as fs } from '@kit.CoreFileKit';
import { pasteboard } from '@kit.BasicServicesKit';
import { ffiInit, ffiSetCallbacks } from '../ohos_ffi';
import { globalEvents, parseEvent } from '../model/RdeskEvents';

const LOG_DOMAIN = 0x5253;
const TAG = 'RustDeskAbility';

export default class EntryAbility extends UIAbility {
  // Whether the system bars are currently hidden for the landscape fullscreen
  // remote-desktop view.
  private systemBarsHidden: boolean = false;

  onDestroy(): void {
    hilog.info(LOG_DOMAIN, TAG, 'onDestroy');
  }

  // Landscape = fullscreen video: hide status/navigation bars; portrait
  // restores them. windowSizeChange also fires for keyboard avoidance resizes,
  // which never cross the aspect boundary, so the state check below keeps the
  // bar toggling to real rotations.
  private updateSystemBars(win: window.Window, w: number, h: number): void {
    const landscape = w > h;
    if (landscape === this.systemBarsHidden) {
      return;
    }
    this.systemBarsHidden = landscape;
    win.setWindowSystemBarEnable(landscape ? [] : ['status', 'navigation'])
      .then(() => {
        hilog.info(LOG_DOMAIN, TAG, 'system bars %{public}s', landscape ? 'hidden' : 'shown');
      })
      .catch((e: BusinessError) => {
        hilog.warn(LOG_DOMAIN, TAG, 'setWindowSystemBarEnable failed: %{public}s', `${e}`);
      });
  }

  onWindowStageCreate(windowStage: window.WindowStage): void {
    // Contract: home_dir is a dedicated subdirectory (created recursively),
    // app_dir is the app sandbox root.
    const filesDir = this.context.filesDir;
    const homeDir = `${filesDir}/home`;
    try {
      fs.mkdirSync(homeDir, true);
    } catch (e) {
      hilog.warn(LOG_DOMAIN, TAG, 'mkdir %{public}s failed: %{public}s', homeDir, `${e}`);
    }
    const rcInit = ffiInit(homeDir, filesDir, '');
    hilog.info(LOG_DOMAIN, TAG, 'ohos_init -> %{public}d', rcInit);
    const rcCb = ffiSetCallbacks((json: string) => {
      globalEvents.dispatch(parseEvent(json));
    }, (text: string) => {
      globalEvents.dispatchClipboard(text);
    });
    hilog.info(LOG_DOMAIN, TAG, 'ohos_set_callbacks -> %{public}d', rcCb);

    // Remote clipboard -> system pasteboard.
    globalEvents.subscribeClipboard((text: string) => {
      try {
        const data = pasteboard.createData(pasteboard.MIMETYPE_TEXT_PLAIN, text);
        pasteboard.getSystemPasteboard().setDataSync(data);
      } catch (e) {
        hilog.warn(LOG_DOMAIN, TAG, 'pasteboard write failed: %{public}s', `${e}`);
      }
    });

    windowStage.loadContent('pages/Index', (err) => {
      if (err.code !== 0) {
        hilog.error(LOG_DOMAIN, TAG, 'loadContent failed: %{public}s', `${err.code}`);
        return;
      }
      // Default keyboard avoidance pans the whole page up, which pushes dialog
      // buttons behind the keyboard. Resizing keeps overlays inside the
      // visible area.
      const win = windowStage.getMainWindowSync();
      win.getUIContext().setKeyboardAvoidMode(KeyboardAvoidMode.RESIZE);
      win.on('windowSizeChange', (size: window.Size) => {
        this.updateSystemBars(win, size.width, size.height);
      });
      try {
        const rect = win.getWindowProperties().windowRect;
        this.updateSystemBars(win, rect.width, rect.height);
      } catch (e) {
        hilog.warn(LOG_DOMAIN, TAG, 'initial windowRect failed: %{public}s', `${e}`);
      }
    });
  }

  onWindowStageDestroy(): void {
    hilog.info(LOG_DOMAIN, TAG, 'onWindowStageDestroy');
  }
}
