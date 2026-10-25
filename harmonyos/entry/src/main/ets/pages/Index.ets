import { router, promptAction } from '@kit.ArkUI';
import { BusinessError } from '@kit.BasicServicesKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { preferences } from '@kit.ArkData';
import { ffiSessionNew, ffiSessionStart, ffiGetOption, ffiSetOption } from '../ohos_ffi';
import { evStr, evBool, parseJsonArray } from '../model/Utils';

const LOG_DOMAIN = 0x5253;
const TAG = 'RustDeskIndex';

// Config option keys consumed by the Rust connection path (see
// docs/harmonyos-migration/ohos-controller-ffi.md).
const OPT_ID_SERVER = 'custom-rendezvous-server';
const OPT_RELAY_SERVER = 'relay-server';
const OPT_API_SERVER = 'api-server';
const OPT_KEY = 'key';

const PREF_STORE = 'rustdesk_index';
const PREF_KEY_RECENT = 'recent_list';
const RECENT_MAX = 10;

class RecentEntry {
  peerId: string = '';
  isFileTransfer: boolean = false;
}

@Entry
@Component
struct Index {
  @State peerIdInput: string = '';
  @State connecting: boolean = false;
  @State connectingId: string = '';
  @State recent: RecentEntry[] = [];
  @State showSettings: boolean = false;
  @State setIdServer: string = '';
  @State setRelayServer: string = '';
  @State setApiServer: string = '';
  @State setKey: string = '';
  private recentStore: preferences.Preferences | undefined = undefined;

  aboutToAppear(): void {
    this.loadRecent();
  }

  private loadRecent(): void {
    preferences.getPreferences(getContext(this), PREF_STORE).then((p: preferences.Preferences) => {
      this.recentStore = p;
      const raw = p.getSync(PREF_KEY_RECENT, '') as string;
      const list: RecentEntry[] = [];
      for (const it of parseJsonArray(raw)) {
        const peerId = evStr(it['peerId']);
        if (peerId.length === 0) {
          continue;
        }
        const e = new RecentEntry();
        e.peerId = peerId;
        e.isFileTransfer = evBool(it['isFileTransfer']);
        list.push(e);
      }
      this.recent = list.slice(0, RECENT_MAX);
    }).catch((e: BusinessError) => {
      hilog.warn(LOG_DOMAIN, TAG, 'load recent failed: %{public}s', `${e}`);
    });
  }

  private saveRecent(): void {
    if (this.recentStore === undefined) {
      return;
    }
    this.recentStore.putSync(PREF_KEY_RECENT, JSON.stringify(this.recent));
    this.recentStore.flush();
  }

  private openSettings(): void {
    this.setIdServer = ffiGetOption(OPT_ID_SERVER);
    this.setRelayServer = ffiGetOption(OPT_RELAY_SERVER);
    this.setApiServer = ffiGetOption(OPT_API_SERVER);
    this.setKey = ffiGetOption(OPT_KEY);
    this.showSettings = true;
  }

  private saveSettings(): void {
    ffiSetOption(OPT_ID_SERVER, this.setIdServer.trim());
    ffiSetOption(OPT_RELAY_SERVER, this.setRelayServer.trim());
    ffiSetOption(OPT_API_SERVER, this.setApiServer.trim());
    ffiSetOption(OPT_KEY, this.setKey.trim());
    this.showSettings = false;
    promptAction.showToast({ message: $r('app.string.settings_saved'), duration: 2000 });
  }

  private addRecent(peerId: string, isFileTransfer: boolean): void {
    const kept = this.recent.filter((r: RecentEntry): boolean => {
      return !(r.peerId === peerId && r.isFileTransfer === isFileTransfer);
    });
    const e = new RecentEntry();
    e.peerId = peerId;
    e.isFileTransfer = isFileTransfer;
    kept.unshift(e);
    this.recent = kept.slice(0, RECENT_MAX);
    this.saveRecent();
  }

  private connect(isFileTransfer: boolean): void {
    // RustDesk IDs are numeric; strip the grouping spaces the IME/user may
    // insert ("26 299 697"), mirroring the desktop clients.
    const pid = this.peerIdInput.replace(/\s+/g, '');
    if (pid.length === 0) {
      promptAction.showToast({ message: $r('app.string.input_peer_id'), duration: 2000 });
      return;
    }
    if (this.connecting) {
      return;
    }
    this.connecting = true;
    this.connectingId = pid;
    const uuid = ffiSessionNew(pid, isFileTransfer, '', false);
    if (uuid.length === 0) {
      this.connecting = false;
      promptAction.showToast({ message: $r('app.string.create_session_failed'), duration: 3000 });
      return;
    }
    const rc = ffiSessionStart(uuid);
    hilog.info(LOG_DOMAIN, TAG, 'session %{public}s start -> %{public}d', uuid, rc);
    this.addRecent(pid, isFileTransfer);
    const url = isFileTransfer ? 'pages/FileTransferPage' : 'pages/RemotePage';
    const params = new SessionParams();
    params.uuid = uuid;
    params.peerId = pid;
    params.isFileTransfer = isFileTransfer;
    router.pushUrl({ url: url, params: params }).catch((e: BusinessError) => {
      hilog.error(LOG_DOMAIN, TAG, 'pushUrl failed: %{public}s', `${e.message}`);
      this.connecting = false;
    });
  }

  onPageShow(): void {
    // Reset when returning from a session page.
    this.connecting = false;
  }

  build() {
    Stack({ alignContent: Alignment.TopEnd }) {
      Column() {
      Text($r('app.string.app_name'))
        .fontSize(26)
        .fontWeight(FontWeight.Bold)
        .margin({ top: 32, bottom: 4 })
      Text($r('app.string.controller_subtitle'))
        .fontSize(13)
        .fontColor('#888888')
        .margin({ bottom: 24 })

      TextInput({ placeholder: $r('app.string.peer_id_hint'), text: this.peerIdInput })
        .type(InputType.Normal)
        .width('86%')
        .height(46)
        .fontSize(18)
        .onChange((v: string) => {
          this.peerIdInput = v;
        })
        .onSubmit(() => {
          this.connect(false);
        })

      Row({ space: 12 }) {
        Button($r('app.string.remote_control'))
          .layoutWeight(1)
          .height(44)
          .onClick(() => {
            this.connect(false);
          })
        Button($r('app.string.file_transfer'))
          .layoutWeight(1)
          .height(44)
          .backgroundColor('#3A7BD5')
          .onClick(() => {
            this.connect(true);
          })
      }
      .width('86%')
      .margin({ top: 12 })

      if (this.connecting) {
        Row({ space: 8 }) {
          LoadingProgress().width(18).height(18)
          Text($r('app.string.connecting')).fontSize(13).fontColor('#666666')
          Text(this.connectingId).fontSize(13).fontColor('#666666')
        }
        .margin({ top: 16 })
      }

      if (this.recent.length > 0) {
        Text($r('app.string.recent_connections'))
          .fontSize(14)
          .fontWeight(FontWeight.Medium)
          .fontColor('#666666')
          .margin({ top: 28, bottom: 6 })
          .width('86%')
        List({ space: 6 }) {
          ForEach(this.recent, (r: RecentEntry) => {
            ListItem() {
              Row({ space: 8 }) {
                Text(r.peerId)
                  .fontSize(16)
                  .layoutWeight(1)
                Text(r.isFileTransfer ? $r('app.string.file_transfer') : $r('app.string.remote_control'))
                  .fontSize(12)
                  .fontColor('#3A7BD5')
              }
              .width('100%')
              .padding({ left: 14, right: 14, top: 12, bottom: 12 })
              .borderRadius(8)
              .backgroundColor('#F2F3F5')
              .onClick(() => {
                this.peerIdInput = r.peerId;
                this.connect(r.isFileTransfer);
              })
            }
          }, (r: RecentEntry) => `${r.peerId}_${r.isFileTransfer ? 1 : 0}`)
        }
        .width('86%')
        .layoutWeight(1)
      }
      }
      .width('100%')
      .height('100%')

      Text($r('app.string.settings'))
        .fontSize(14)
        .fontColor('#3A7BD5')
        .padding({ top: 20, bottom: 8, left: 12, right: 20 })
        .onClick(() => {
          this.openSettings();
        })

      if (this.showSettings) {
        Column() {
          Column() {
            Text($r('app.string.network_settings'))
              .fontSize(18)
              .fontWeight(FontWeight.Medium)
            Text($r('app.string.network_settings_hint'))
              .fontSize(12)
              .fontColor('#999999')
              .margin({ top: 6, bottom: 14 })

            Column({ space: 12 }) {
              Column() {
                Text($r('app.string.id_server'))
                  .fontSize(13)
                  .fontColor('#666666')
                  .margin({ bottom: 4 })
                TextInput({ placeholder: $r('app.string.id_server_hint'), text: this.setIdServer })
                  .type(InputType.Normal)
                  .width('100%')
                  .height(42)
                  .fontSize(15)
                  .onChange((v: string) => {
                    this.setIdServer = v;
                  })
              }
              .width('100%')
              .alignItems(HorizontalAlign.Start)

              Column() {
                Text($r('app.string.relay_server'))
                  .fontSize(13)
                  .fontColor('#666666')
                  .margin({ bottom: 4 })
                TextInput({ placeholder: $r('app.string.relay_server_hint'), text: this.setRelayServer })
                  .type(InputType.Normal)
                  .width('100%')
                  .height(42)
                  .fontSize(15)
                  .onChange((v: string) => {
                    this.setRelayServer = v;
                  })
              }
              .width('100%')
              .alignItems(HorizontalAlign.Start)

              Column() {
                Text($r('app.string.api_server'))
                  .fontSize(13)
                  .fontColor('#666666')
                  .margin({ bottom: 4 })
                TextInput({ placeholder: $r('app.string.api_server_hint'), text: this.setApiServer })
                  .type(InputType.Normal)
                  .width('100%')
                  .height(42)
                  .fontSize(15)
                  .onChange((v: string) => {
                    this.setApiServer = v;
                  })
              }
              .width('100%')
              .alignItems(HorizontalAlign.Start)

              Column() {
                Text($r('app.string.server_key'))
                  .fontSize(13)
                  .fontColor('#666666')
                  .margin({ bottom: 4 })
                TextInput({ placeholder: $r('app.string.server_key_hint'), text: this.setKey })
                  .type(InputType.Normal)
                  .width('100%')
                  .height(42)
                  .fontSize(15)
                  .onChange((v: string) => {
                    this.setKey = v;
                  })
              }
              .width('100%')
              .alignItems(HorizontalAlign.Start)
            }
            .width('100%')

            Row({ space: 12 }) {
              Button($r('app.string.cancel'), { buttonStyle: ButtonStyleMode.NORMAL })
                .layoutWeight(1)
                .height(40)
                .onClick(() => {
                  this.showSettings = false;
                })
              Button($r('app.string.save'))
                .layoutWeight(1)
                .height(40)
                .backgroundColor('#3A7BD5')
                .onClick(() => {
                  this.saveSettings();
                })
            }
            .width('100%')
            .margin({ top: 16 })
          }
          .width('82%')
          .padding(16)
          .borderRadius(12)
          .backgroundColor('#FFFFFF')
        }
        .width('100%')
        .height('100%')
        .backgroundColor('rgba(0,0,0,0.45)')
        .justifyContent(FlexAlign.Center)
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor('#FFFFFF')
  }
}

class SessionParams {
  uuid: string = '';
  peerId: string = '';
  isFileTransfer: boolean = false;
}
