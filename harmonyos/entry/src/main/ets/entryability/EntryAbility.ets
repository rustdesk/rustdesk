import { UIAbility, abilityAccessCtrl, PermissionRequestResult,
  Permissions } from '@kit.AbilityKit';
import { window, KeyboardAvoidMode, router } from '@kit.ArkUI';
import { BusinessError } from '@kit.BasicServicesKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { fileIo as fs } from '@kit.CoreFileKit';
import { pasteboard } from '@kit.BasicServicesKit';
import { image } from '@kit.ImageKit';
import { promptAction } from '@kit.ArkUI';
import { util } from '@kit.ArkTS';
import { ffiInit, ffiSetCallbacks, ffiServerStart,
  ffiCmHandleIncomingVoiceCall } from '../ohos_ffi';
import { globalEvents, parseEvent, RdeskEvent } from '../model/RdeskEvents';
import { evStr, evNum, evBool } from '../model/Utils';

const LOG_DOMAIN = 0x5253;
const TAG = 'RustDeskAbility';

class ClientParams {
  client: string = '';
  name: string = '';
}

export default class EntryAbility extends UIAbility {
  // Whether the system bars are currently hidden for the landscape fullscreen
  // remote-desktop view.
  private systemBarsHidden: boolean = false;
  // Id of the connection whose incoming voice call is currently offered in the
  // global dialog; -1 when no dialog is up.
  private voiceCallDialogConnId: number = -1;
  // Connection id -> peer name, from cm_add_connection, for titling the chat
  // page opened automatically on a peer message.
  private peerNames: Map<number, string> = new Map();
  // Router stack already holds the auto-opened chat page for this connection.
  private autoChatConnId: number = -1;

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
    // Register with the rendezvous server as soon as the app is open, so the
    // SharePage ID resolves even before 开始共享 starts the capture. Screen
    // capture itself stays gated behind the share button.
    const rcServer = ffiServerStart();
    hilog.info(LOG_DOMAIN, TAG, 'ohos_server_start -> %{public}d', rcServer);
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

    // Controlled side: a remote peer's clipboard (text / html / png) lands on
    // the system pasteboard whenever the connection pushes it.
    globalEvents.subscribe((ev: RdeskEvent) => {
      if (ev.name !== 'cm_clipboard_write') {
        return;
      }
      const text = ev.get('text');
      const html = ev.get('html');
      const png = ev.get('png');
      if (png.length > 0) {
        this.writePasteboardImage(png);
        return;
      }
      if (text.length === 0 && html.length === 0) {
        return;
      }
      try {
        const data = pasteboard.createData(pasteboard.MIMETYPE_TEXT_PLAIN,
                                           text.length > 0 ? text : html);
        if (html.length > 0) {
          data.addRecord(pasteboard.MIMETYPE_TEXT_HTML, html);
        }
        pasteboard.getSystemPasteboard().setDataSync(data);
      } catch (e) {
        hilog.warn(LOG_DOMAIN, TAG, 'cm pasteboard write failed: %{public}s', `${e}`);
      }
    });

    // Peer names for titling the auto-opened chat page.
    globalEvents.subscribe((ev: RdeskEvent) => {
      if (ev.name !== 'cm_add_connection') {
        return;
      }
      try {
        const obj: Object = JSON.parse(ev.get('client')) as Object;
        const rec: Record<string, Object> = obj as Record<string, Object>;
        this.peerNames.set(evNum(evStr(rec['id'])), evStr(rec['name']));
      } catch (e) {
        hilog.warn(LOG_DOMAIN, TAG, 'parse client name failed: %{public}s', `${e}`);
      }
    });

    // Voice call surfaces live here rather than in a page: the controlled
    // side's pages come and go with the router, while an incoming call must be
    // answerable wherever the user happens to be. SharePage additionally
    // renders the state on its connection cards when it is visible.
    globalEvents.subscribe((ev: RdeskEvent) => {
      if (ev.name !== 'cm_voice_call_state') {
        return;
      }
      let id = -1;
      let name = '';
      let incoming = false;
      try {
        const obj: Object = JSON.parse(ev.get('client')) as Object;
        const rec: Record<string, Object> = obj as Record<string, Object>;
        id = evNum(evStr(rec['id']));
        name = evStr(rec['name']);
        incoming = evBool(evStr(rec['incoming_voice_call']));
      } catch (e) {
        hilog.warn(LOG_DOMAIN, TAG, 'parse voice call client failed: %{public}s', `${e}`);
        return;
      }
      if (incoming) {
        this.showVoiceCallDialog(id, name);
      } else {
        // Call started or ended elsewhere (SharePage card, peer hang-up): the
        // modal offer above no longer matches reality; drop it so the next
        // incoming call re-opens a fresh one.
        this.voiceCallDialogConnId = -1;
      }
    });

    // A peer chat message pops the conversation open right away: the user
    // should not have to find the SharePage card to read it. ChatPage renders
    // the message itself; opening is skipped when it is already on top.
    globalEvents.subscribe((ev: RdeskEvent) => {
      if (ev.name !== 'cm_chat') {
        return;
      }
      const id = ev.num('id');
      try {
        const state = router.getState();
        if (state.name === 'pages/ChatPage'
          && evNum(evStr((state.params as Record<string, Object>)['id'])) === id) {
          return;
        }
      } catch (e) {
        // Fall through: an unavailable router state still opens the chat.
      }
      this.openChatPage(id);
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

  // Open the conversation for one connection. Only auto-opens once per
  // connection: after the user leaves the page, further messages go back to
  // the unread badge on the SharePage card.
  private openChatPage(id: number): void {
    if (this.autoChatConnId === id) {
      return;
    }
    this.autoChatConnId = id;
    const params = new ClientParams();
    params.client = `${id}`;
    params.name = this.peerNames.get(id) ?? '';
    router.pushUrl({ url: 'pages/ChatPage', params: params })
      .then(() => {
        this.autoChatConnId = -1;
      })
      .catch((e: BusinessError) => {
        hilog.warn(LOG_DOMAIN, TAG, 'auto open ChatPage failed: %{public}s', `${e.message}`);
        this.autoChatConnId = -1;
      });
  }

  // Global incoming-voice-call dialog. Answers with the same mic-permission
  // flow as the SharePage card; a denial declines so the peer is not left
  // waiting on a one-way call.
  private showVoiceCallDialog(id: number, name: string): void {
    if (this.voiceCallDialogConnId === id) {
      return;
    }
    this.voiceCallDialogConnId = id;
    promptAction.showDialog({
      title: $r('app.string.share_voice_call_incoming'),
      message: name.length > 0 ? name : `#${id}`,
      buttons: [
        { text: $r('app.string.share_reject'), color: '#E5484D' },
        { text: $r('app.string.share_accept'), color: '#1A6AF5' },
      ],
    })
      .then((r: promptAction.ShowDialogSuccessResponse) => {
        this.voiceCallDialogConnId = -1;
        if (r.index === 1) {
          this.acceptVoiceCall(id);
        } else {
          ffiCmHandleIncomingVoiceCall(id, false);
        }
      })
      .catch((e: BusinessError) => {
        hilog.warn(LOG_DOMAIN, TAG, 'voice call dialog failed: %{public}s', `${e}`);
        this.voiceCallDialogConnId = -1;
      });
  }

  private acceptVoiceCall(id: number): void {
    const perms: Array<Permissions> = ['ohos.permission.MICROPHONE'];
    abilityAccessCtrl.createAtManager()
      .requestPermissionsFromUser(this.context, perms)
      .then((result: PermissionRequestResult) => {
        const granted = result.authResults.every((r: number): boolean => r === 0);
        this.voiceCallDialogConnId = -1;
        if (!granted) {
          hilog.warn(LOG_DOMAIN, TAG, 'mic permission denied for voice call');
          ffiCmHandleIncomingVoiceCall(id, false);
          return;
        }
        ffiCmHandleIncomingVoiceCall(id, true);
      })
      .catch((e: BusinessError) => {
        hilog.warn(LOG_DOMAIN, TAG, 'request mic permission failed: %{public}s', `${e}`);
        this.voiceCallDialogConnId = -1;
        ffiCmHandleIncomingVoiceCall(id, false);
      });
  }

  // Decode a base64 PNG clipboard payload and publish it as a pixel map so
  // any app can paste it (same flow as the controller-side clipboard_image).
  private writePasteboardImage(pngBase64: string): void {
    if (pngBase64.length === 0) {
      return;
    }
    try {
      const bytes = new util.Base64Helper().decodeSync(pngBase64);
      const buf = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
      const src: image.ImageSource = image.createImageSource(buf);
      src.createPixelMap().then((pm: image.PixelMap) => {
        try {
          const data = pasteboard.createData(pasteboard.MIMETYPE_PIXELMAP, pm);
          pasteboard.getSystemPasteboard().setData(data).catch((e: BusinessError) => {
            hilog.warn(LOG_DOMAIN, TAG, 'paste image failed: %{public}s', `${e}`);
          });
        } catch (e) {
          hilog.warn(LOG_DOMAIN, TAG, 'pasteboard create image failed: %{public}s', `${e}`);
        }
      }).catch((e: BusinessError) => {
        hilog.warn(LOG_DOMAIN, TAG, 'createPixelMap failed: %{public}s', `${e}`);
      });
    } catch (e) {
      hilog.warn(LOG_DOMAIN, TAG, 'clipboard image decode failed: %{public}s', `${e}`);
    }
  }

  onWindowStageDestroy(): void {
    hilog.info(LOG_DOMAIN, TAG, 'onWindowStageDestroy');
  }
}
