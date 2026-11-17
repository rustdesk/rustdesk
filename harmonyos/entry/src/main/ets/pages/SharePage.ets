// Controlled-side (被控端) page: shows the device ID / password, starts and
// stops the share service (rendezvous registration + AVScreenCapture) and
// handles incoming connection requests delivered as cm_* events.
//
// Rust side: src/ohos_ffi.rs (server C ABI), connection manager events via
// the shared event JSON channel (docs/harmonyos-migration/ohos-controller-ffi.md
// §3, cm_* additions).

import { router, promptAction, display } from '@kit.ArkUI';import { BusinessError, pasteboard } from '@kit.BasicServicesKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { backgroundTaskManager } from '@kit.BackgroundTasksKit';
import { notificationManager } from '@kit.NotificationKit';
import { abilityAccessCtrl, common, wantAgent, WantAgent,
  PermissionRequestResult, Permissions } from '@kit.AbilityKit';
import {
  ffiServerStart, ffiServerStartService, ffiServerStopService,
  ffiGetServerId, ffiGetServerPassword, ffiUpdateServerPassword,
  ffiHasPermanentPassword, ffiServerAuthorize, ffiServerCloseConnection,
  ffiCaptureStart, ffiCaptureStop, ffiCaptureRunning,
  ffiCmHandleIncomingVoiceCall, ffiCmCloseVoiceCall,
  ffiGetClients,
} from '../ohos_ffi';
import { globalEvents, parseEvent, RdeskEvent } from '../model/RdeskEvents';
import { evBool, evNum, evStr, parseJsonArray } from '../model/Utils';

const LOG_DOMAIN = 0x5253;
const TAG = 'RustDeskShare';

// OH_AVScreenCaptureStateCode (subset surfaced by the Rust layer).
const CAPTURE_STATE_STARTED = 0;
const CAPTURE_STATE_CANCELED = 1;
const CAPTURE_STATE_STOPPED_BY_USER = 2;
const CAPTURE_STATE_INTERRUPTED = 3;
const CAPTURE_STATE_STOPPED_BY_CALL = 4;
const CAPTURE_STATE_PENDING = -1;
const CAPTURE_STATE_STOPPED_BY_SWITCHES = 10;

class ClientInfo {
  id: number = 0;
  authorized: boolean = false;
  name: string = '';
  peerId: string = '';
  isFileTransfer: boolean = false;
  disconnected: boolean = false;
  incomingVoiceCall: boolean = false;
  inVoiceCall: boolean = false;
  // Unread peer messages: bumped by cm_chat while this page is visible,
  // cleared when the chat page for the connection is opened.
  unreadChat: number = 0;
}

class ClientParams {
  client: string = '';
  name: string = '';
}

@Entry
@Component
struct SharePage {
  @State deviceId: string = '';
  @State password: string = '';
  @State hasPermanent: boolean = false;
  @State serviceRunning: boolean = false;
  @State captureRunning: boolean = false;
  @State captureStarting: boolean = false;
  @State pending: ClientInfo[] = [];
  @State clients: ClientInfo[] = [];
  private handler = (ev: RdeskEvent): void => {
    this.onEvent(ev);
  };

  aboutToAppear(): void {
    this.deviceId = ffiGetServerId();
    this.hasPermanent = ffiHasPermanentPassword() === 1;
    this.refreshPassword();
    this.captureRunning = ffiCaptureRunning() === 1;
    this.serviceRunning = this.captureRunning;
    globalEvents.subscribe(this.handler);
    // Requests that arrived while this page was closed are already in the
    // CM state; pull them so they are not lost to the missed live events.
    try {
      for (const it of parseJsonArray(ffiGetClients())) {
        const c = this.parseClient(JSON.stringify(it));
        if (c !== null) {
          this.upsertClient(c);
        }
      }
    } catch (e) {
      hilog.warn(LOG_DOMAIN, TAG, 'fetch clients failed: %{public}s', `${e}`);
    }
  }

  aboutToDisappear(): void {
    globalEvents.unsubscribe(this.handler);
  }

  private refreshPassword(): void {
    this.password = ffiGetServerPassword();
  }

  private onEvent(ev: RdeskEvent): void {
    switch (ev.name) {
      case 'cm_add_connection': {
        const c = this.parseClient(ev.get('client'));
        if (c === null) {
          return;
        }
        this.upsertClient(c);
        break;
      }
      case 'cm_chat': {
        // Chat surfaces live on the connection card (unread badge + chat
        // button); ChatPage owns the conversation while open. When ChatPage is
        // on top of the router stack it renders the messages itself, so the
        // card behind it does not count unread badges.
        const id = ev.num('id');
        let inChat = false;
        try {
          const state = router.getState();
          inChat = state.name === 'pages/ChatPage'
            && evNum(evStr((state.params as Record<string, Object>)['id'])) === id;
        } catch (e) {
          inChat = false;
        }
        if (inChat) {
          break;
        }
        let hit = false;
        this.clients = this.clients.map((p: ClientInfo): ClientInfo => {
          if (p.id === id) {
            hit = true;
            const c = this.copyClient(p);
            c.unreadChat = p.unreadChat + 1;
            return c;
          }
          return p;
        });
        if (!hit) {
          hilog.warn(LOG_DOMAIN, TAG, 'chat for unknown connection %{public}d', id);
        }
        break;
      }
      case 'cm_remove_connection': {
        const id = ev.num('id');
        this.pending = this.pending.filter((c: ClientInfo): boolean => c.id !== id);
        this.clients = this.clients.filter((c: ClientInfo): boolean => c.id !== id);
        break;
      }
      case 'cm_voice_call_state': {
        // The full client record rides along; the flags drive the incoming /
        // in-call surfaces on the connection cards.
        const c = this.parseClient(ev.get('client'));
        if (c === null) {
          return;
        }
        this.upsertClient(c);
        break;
      }
      case 'server_capture_state': {
        this.onCaptureState(ev.num('state'));
        break;
      }
      default:
        break;
    }
  }

  private parseClient(s: string): ClientInfo | null {
    let c = new ClientInfo();
    try {
      const obj: Object = JSON.parse(s) as Object;
      const rec: Record<string, Object> = obj as Record<string, Object>;
      c.id = evNum(evStr(rec['id']));
      c.authorized = evBool(evStr(rec['authorized']));
      c.name = evStr(rec['name']);
      c.peerId = evStr(rec['peer_id']);
      c.isFileTransfer = evBool(evStr(rec['is_file_transfer']));
      c.disconnected = evBool(evStr(rec['disconnected']));
      c.incomingVoiceCall = evBool(evStr(rec['incoming_voice_call']));
      c.inVoiceCall = evBool(evStr(rec['in_voice_call']));
    } catch (e) {
      hilog.warn(LOG_DOMAIN, TAG, 'parse client failed: %{public}s', `${e}`);
      return null;
    }
    return c;
  }

  private upsertClient(c: ClientInfo): void {
    this.pending = this.pending.filter((p: ClientInfo): boolean => p.id !== c.id);
    this.clients = this.clients.filter((p: ClientInfo): boolean => p.id !== c.id);
    if (c.disconnected) {
      return;
    }
    if (c.authorized) {
      this.clients.push(c);
    } else {
      this.pending.push(c);
    }
  }

  // ohos.permission.MICROPHONE is user_grant: accepting an incoming call
  // requests it first; a denial declines the call so the peer is not left
  // waiting on a one-way call.
  private clearUnread(id: number): void {
    this.clients = this.clients.map((p: ClientInfo): ClientInfo => {
      if (p.id !== id) {
        return p;
      }
      const c = this.copyClient(p);
      c.unreadChat = 0;
      return c;
    });
  }

  private acceptVoiceCall(id: number): void {
    const perms: Array<Permissions> = ['ohos.permission.MICROPHONE'];
    abilityAccessCtrl.createAtManager().requestPermissionsFromUser(getContext(this), perms)
      .then((result: PermissionRequestResult) => {
        const granted = result.authResults.every((r: number): boolean => r === 0);
        if (!granted) {
          promptAction.showToast({ message: $r('app.string.mic_denied'), duration: 2500 });
          ffiCmHandleIncomingVoiceCall(id, false);
          return;
        }
        ffiCmHandleIncomingVoiceCall(id, true);
      })
      .catch((e: BusinessError) => {
        hilog.warn(LOG_DOMAIN, TAG, 'request mic permission failed: %{public}s', `${e}`);
        ffiCmHandleIncomingVoiceCall(id, false);
      });
  }

  private copyClient(p: ClientInfo): ClientInfo {
    const c = new ClientInfo();
    c.id = p.id;
    c.authorized = p.authorized;
    c.name = p.name;
    c.peerId = p.peerId;
    c.isFileTransfer = p.isFileTransfer;
    c.disconnected = p.disconnected;
    c.incomingVoiceCall = p.incomingVoiceCall;
    c.inVoiceCall = p.inVoiceCall;
    c.unreadChat = p.unreadChat;
    return c;
  }

  private onCaptureState(state: number): void {
    hilog.info(LOG_DOMAIN, TAG, 'capture state %{public}d', state);
    if (state === CAPTURE_STATE_PENDING) {
      // The consent picker is still on screen; keep the starting indicator.
      return;
    }
    this.captureStarting = false;
    if (state === CAPTURE_STATE_STARTED) {
      this.captureRunning = true;
      this.serviceRunning = true;
      promptAction.showToast({ message: $r('app.string.share_capture_started'), duration: 2000 });
    } else {
      this.captureRunning = false;
      this.serviceRunning = false;
      let msg = $r('app.string.share_capture_stopped');
      if (state === CAPTURE_STATE_CANCELED) {
        msg = $r('app.string.share_capture_canceled');
      } else if (state === CAPTURE_STATE_INTERRUPTED) {
        msg = $r('app.string.share_capture_interrupted');
      }
      promptAction.showToast({ message: msg, duration: 2500 });
    }
  }

  private async startContinuousTask(): Promise<void> {
    // The continuous task posts a status-bar notification; without notification
    // authorization startBackgroundRunning fails (9800006) and the process is
    // frozen shortly after backgrounding, dropping every connection.
    await notificationManager.requestEnableNotification(
      getContext(this) as common.UIAbilityContext);
    const wantAgentObj: WantAgent =
      await wantAgent.getWantAgent({
        wants: [{ bundleName: 'com.rustdesk.ohos', abilityName: 'EntryAbility' }],
        operationType: wantAgent.OperationType.START_ABILITY,
        requestCode: 0,
      });
    await backgroundTaskManager.startBackgroundRunning(
      getContext(this), backgroundTaskManager.BackgroundMode.DATA_TRANSFER, wantAgentObj);
    hilog.info(LOG_DOMAIN, TAG, 'continuous task started');
  }

  private stopContinuousTask(): void {
    backgroundTaskManager.stopBackgroundRunning(getContext(this)).catch((e: BusinessError) => {
      hilog.warn(LOG_DOMAIN, TAG, 'stopBackgroundRunning failed: %{public}s', `${e}`);
    });
  }

  private startService(): void {
    if (this.captureStarting || this.captureRunning) {
      return;
    }
    const rcServer = ffiServerStart();
    hilog.info(LOG_DOMAIN, TAG, 'server start -> %{public}d', rcServer);
    this.captureStarting = true;
    // Notification consent (part of the continuous task) must complete before
    // the capture consent dialog pops, so the two system dialogs never stack.
    this.startContinuousTask().catch((e: BusinessError) => {
      hilog.warn(LOG_DOMAIN, TAG, 'continuous task unavailable: %{public}d %{public}s',
                 e.code, e.message);
      promptAction.showToast({ message: $r('app.string.share_background_failed'),
        duration: 4000 });
    }).then(() => {
      this.startCapture();
    });
  }

  private startCapture(): void {
    const disp = display.getDefaultDisplaySync();
    const rc = ffiCaptureStart(disp.width, disp.height);
    hilog.info(LOG_DOMAIN, TAG, 'capture start %{public}dx%{public}d -> %{public}d',
      disp.width, disp.height, rc);
    if (rc !== 0) {
      this.captureStarting = false;
      this.stopContinuousTask();
      promptAction.showToast({ message: $r('app.string.share_capture_start_failed'),
        duration: 3000 });
      return;
    }
    // The mediator only runs when the stop-service flag is clear.
    ffiServerStartService();
  }

  private stopService(): void {
    ffiCaptureStop();
    ffiServerStopService();
    this.stopContinuousTask();
    this.captureRunning = false;
    this.serviceRunning = false;
    this.pending = [];
    this.clients = [];
  }

  private copyText(text: string): void {
    try {
      const pasteData = pasteboard.createData(pasteboard.MIMETYPE_TEXT_PLAIN, text);
      pasteboard.getSystemPasteboard().setDataSync(pasteData);
      promptAction.showToast({ message: $r('app.string.copied'), duration: 1500 });
    } catch (e) {
      hilog.warn(LOG_DOMAIN, TAG, 'copy failed: %{public}s', `${e}`);
    }
  }

  build() {
    Column() {
      Row() {
        Text($r('app.string.back'))
          .fontSize(16)
          .fontColor('#1A6AF5')
          .onClick(() => {
            router.back();
          })
        Blank()
        Text($r('app.string.share_title'))
          .fontSize(18)
          .fontWeight(FontWeight.Bold)
        Blank()
        Text('')
          .fontSize(16)
      }
      .width('100%')
      .padding({ left: 16, right: 16, top: 16, bottom: 8 })

      Scroll() {
        Column() {
          // ID card.
          Column() {
            Text($r('app.string.share_id_label'))
              .fontSize(13)
              .fontColor('#888888')
            Row({ space: 8 }) {
              Text(this.deviceId.length > 0 ? this.deviceId : '-')
                .fontSize(30)
                .fontWeight(FontWeight.Bold)
                .letterSpacing(2)
              SymbolGlyph($r('sys.symbol.square_on_square_fill'))
                .fontSize(20)
                .fontColor(['#1A6AF5'])
                .onClick(() => {
                  this.copyText(this.deviceId);
                })
            }
            .margin({ top: 4 })
          }
          .width('100%')
          .padding(16)
          .borderRadius(12)
          .backgroundColor('#F2F4F8')
          .margin({ top: 8 })

          // Password card.
          Column() {
            Row() {
              Text($r('app.string.share_password_label'))
                .fontSize(13)
                .fontColor('#888888')
              Blank()
              if (this.hasPermanent) {
                Text($r('app.string.share_permanent_password_badge'))
                  .fontSize(11)
                  .fontColor('#1A6AF5')
                  .border({ width: 1, color: '#1A6AF5', radius: 4 })
                  .padding({ left: 6, right: 6, top: 2, bottom: 2 })
              }
            }
            .width('100%')
            Row({ space: 8 }) {
              Text(this.password.length > 0 ? this.password : '-')
                .fontSize(24)
                .fontWeight(FontWeight.Medium)
                .letterSpacing(2)
              SymbolGlyph($r('sys.symbol.square_on_square_fill'))
                .fontSize(20)
                .fontColor(['#1A6AF5'])
                .onClick(() => {
                  this.copyText(this.password);
                })
              Blank()
              Text($r('app.string.share_password_refresh'))
                .fontSize(13)
                .fontColor('#1A6AF5')
                .onClick(() => {
                  ffiUpdateServerPassword();
                  this.refreshPassword();
                })
            }
            .width('100%')
            .margin({ top: 4 })
          }
          .width('100%')
          .padding(16)
          .borderRadius(12)
          .backgroundColor('#F2F4F8')
          .margin({ top: 10 })

          // Service control.
          Column() {
            Row({ space: 8 }) {
              Circle({ width: 10, height: 10 })
                .fill(this.captureRunning ? '#34C759' : '#C7C7CC')
              Text(this.captureStarting ? $r('app.string.share_waiting_consent')
                : (this.captureRunning ? $r('app.string.share_service_running')
                  : $r('app.string.share_service_stopped')))
                .fontSize(14)
                .fontColor('#444444')
            }
            Button(this.captureRunning ? $r('app.string.share_stop')
              : $r('app.string.share_start'))
              .width('100%')
              .height(46)
              .fontSize(17)
              .backgroundColor(this.captureRunning ? '#E5484D' : '#1A6AF5')
              .margin({ top: 12 })
              .enabled(!this.captureStarting)
              .onClick(() => {
                if (this.captureRunning) {
                  this.stopService();
                } else {
                  this.startService();
                }
              })
            Text(this.captureRunning ? $r('app.string.share_start_hint')
              : $r('app.string.share_id_reachable_hint'))
              .fontSize(12)
              .fontColor('#999999')
              .margin({ top: 8 })
          }
          .width('100%')
          .margin({ top: 14 })

          // Pending connection requests.
          if (this.pending.length > 0) {
            Text($r('app.string.share_requests'))
              .fontSize(15)
              .fontWeight(FontWeight.Medium)
              .margin({ top: 18, bottom: 6 })
              .width('100%')
            ForEach(this.pending, (c: ClientInfo) => {
              Column() {
                Text(`${c.name} (${c.peerId})`)
                  .fontSize(15)
                  .fontWeight(FontWeight.Medium)
                Text(c.isFileTransfer ? $r('app.string.file_transfer')
                  : $r('app.string.remote_control'))
                  .fontSize(12)
                  .fontColor('#888888')
                  .margin({ top: 2 })
                Row({ space: 12 }) {
                  Button($r('app.string.share_reject'))
                    .layoutWeight(1)
                    .height(38)
                    .backgroundColor('#E5484D')
                    .onClick(() => {
                      ffiServerAuthorize(c.id, false);
                      this.pending = this.pending.filter(
                        (p: ClientInfo): boolean => p.id !== c.id);
                    })
                  Button($r('app.string.share_accept'))
                    .layoutWeight(1)
                    .height(38)
                    .backgroundColor('#1A6AF5')
                    .onClick(() => {
                      ffiServerAuthorize(c.id, true);
                      const accepted = new ClientInfo();
                      accepted.id = c.id;
                      accepted.authorized = true;
                      accepted.name = c.name;
                      accepted.peerId = c.peerId;
                      accepted.isFileTransfer = c.isFileTransfer;
                      this.pending = this.pending.filter(
                        (p: ClientInfo): boolean => p.id !== c.id);
                      this.clients.push(accepted);
                    })
                }
                .width('100%')
                .margin({ top: 10 })
              }
              .width('100%')
              .padding(12)
              .borderRadius(10)
              .backgroundColor('#FFF7E6')
              .margin({ bottom: 8 })
            }, (c: ClientInfo) => `${c.id}`)
          }

          // Active connections.
          if (this.clients.length > 0) {
            Text($r('app.string.share_connected'))
              .fontSize(15)
              .fontWeight(FontWeight.Medium)
              .margin({ top: 18, bottom: 6 })
              .width('100%')
            ForEach(this.clients, (c: ClientInfo) => {
              Column() {
                Row() {
                  Column() {
                    Row({ space: 6 }) {
                      Text(`${c.name} (${c.peerId})`)
                        .fontSize(15)
                      if (c.unreadChat > 0) {
                        Text(`${c.unreadChat}`)
                          .fontSize(11)
                          .fontColor('#FFFFFF')
                          .backgroundColor('#E5484D')
                          .borderRadius(9)
                          .padding({ left: 5, right: 5, top: 1, bottom: 1 })
                      }
                    }
                    Text(`#${c.id}`)
                      .fontSize(11)
                      .fontColor('#999999')
                  }
                  .alignItems(HorizontalAlign.Start)
                  .layoutWeight(1)
                  Button() {
                    SymbolGlyph($r('sys.symbol.message'))
                      .fontSize(22)
                      .fontColor(['#1A6AF5'])
                  }
                  .type(ButtonType.Circle)
                  .width(38)
                  .height(38)
                  .backgroundColor('#E8F0FE')
                  .accessibilityText($r('app.string.chat_title'))
                  .onClick(() => {
                    const params = new ClientParams();
                    params.client = `${c.id}`;
                    params.name = c.name;
                    router.pushUrl({ url: 'pages/ChatPage', params: params })
                      .catch((e: BusinessError) => {
                        hilog.error(LOG_DOMAIN, TAG, 'pushUrl ChatPage failed: %{public}s',
                          `${e.message}`);
                      });
                    this.clearUnread(c.id);
                  })
                  if (c.inVoiceCall || c.incomingVoiceCall) {
                    Button() {
                      SymbolGlyph(c.inVoiceCall ? $r('sys.symbol.phone_down_fill')
                        : $r('sys.symbol.phone'))
                        .fontSize(22)
                        .fontColor([c.inVoiceCall ? '#E5484D' : '#34C759'])
                    }
                    .type(ButtonType.Circle)
                    .width(38)
                    .height(38)
                    .backgroundColor(c.inVoiceCall ? '#FDECEC' : '#E9F9EF')
                    .margin({ left: 8 })
                    .accessibilityText(c.inVoiceCall ? $r('app.string.voice_call_end')
                      : $r('app.string.share_voice_call_incoming'))
                    .onClick(() => {
                      if (c.inVoiceCall) {
                        ffiCmCloseVoiceCall(c.id);
                      } else {
                        // Green phone while ringing: answer, matching the glyph.
                        ffiCmHandleIncomingVoiceCall(c.id, true);
                      }
                    })
                  }
                  Button() {
                    SymbolGlyph($r('sys.symbol.xmark'))
                      .fontSize(20)
                      .fontColor(['#E5484D'])
                  }
                  .type(ButtonType.Circle)
                  .width(38)
                  .height(38)
                  .backgroundColor('#FDECEC')
                  .margin({ left: 8 })
                  .accessibilityText($r('app.string.share_disconnect'))
                  .onClick(() => {
                    ffiServerCloseConnection(c.id);
                    this.clients = this.clients.filter(
                      (p: ClientInfo): boolean => p.id !== c.id);
                  })
                }
                .width('100%')
                if (c.incomingVoiceCall) {
                  Row({ space: 8 }) {
                    SymbolGlyph($r('sys.symbol.phone'))
                      .fontSize(16)
                      .fontColor(['#B25E00'])
                    Text($r('app.string.share_voice_call_incoming'))
                      .fontSize(13)
                      .fontColor('#B25E00')
                      .layoutWeight(1)
                    Button() {
                      SymbolGlyph($r('sys.symbol.xmark'))
                        .fontSize(18)
                        .fontColor(['#E5484D'])
                    }
                    .type(ButtonType.Circle)
                    .width(34)
                    .height(34)
                    .backgroundColor('#FDECEC')
                    .accessibilityText($r('app.string.share_reject'))
                    .onClick(() => {
                      ffiCmHandleIncomingVoiceCall(c.id, false);
                    })
                    Button() {
                      SymbolGlyph($r('sys.symbol.phone'))
                        .fontSize(18)
                        .fontColor(['#FFFFFF'])
                    }
                    .type(ButtonType.Circle)
                    .width(34)
                    .height(34)
                    .backgroundColor('#34C759')
                    .accessibilityText($r('app.string.share_accept'))
                    .onClick(() => {
                      this.acceptVoiceCall(c.id);
                    })
                  }
                  .width('100%')
                  .margin({ top: 10 })
                } else if (c.inVoiceCall) {
                  Row({ space: 8 }) {
                    Circle({ width: 8, height: 8 })
                      .fill('#34C759')
                    Text($r('app.string.share_voice_call_in_progress'))
                      .fontSize(13)
                      .fontColor('#444444')
                      .layoutWeight(1)
                    Button() {
                      SymbolGlyph($r('sys.symbol.phone_down_fill'))
                        .fontSize(20)
                        .fontColor(['#E5484D'])
                    }
                    .type(ButtonType.Circle)
                    .width(34)
                    .height(34)
                    .backgroundColor('#FDECEC')
                    .accessibilityText($r('app.string.voice_call_end'))
                    .onClick(() => {
                      ffiCmCloseVoiceCall(c.id);
                    })
                  }
                  .width('100%')
                  .margin({ top: 10 })
                }
              }
              .width('100%')
              .padding(12)
              .borderRadius(10)
              .backgroundColor(c.incomingVoiceCall ? '#FFF7E6' : '#F2F4F8')
              .margin({ bottom: 8 })
            }, (c: ClientInfo) =>
              `${c.id}-${c.incomingVoiceCall}-${c.inVoiceCall}-${c.unreadChat}`)
          }

          Blank().height(24)
        }
        .width('88%')
        .alignItems(HorizontalAlign.Start)
      }
      .layoutWeight(1)
      .align(Alignment.Top)
      .scrollBar(BarState.Off)
    }
    .width('100%')
    .height('100%')
    .backgroundColor('#FFFFFF')
  }
}
