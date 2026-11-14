// Controlled-side (被控端) chat page: shows the conversation with one remote
// peer and sends replies back over the session channel.
//
// Rust side: incoming messages arrive as `cm_chat` events (id + text) pushed
// by OhosCMHandler::new_message; outgoing text goes through
// ohos_server_send_chat -> ui_cm_interface::send_chat -> Data::ChatMessage.
// Navigation carries the connection id + peer name, so events for other
// connections are filtered here.

import { router, promptAction } from '@kit.ArkUI';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { ffiServerSendChat } from '../ohos_ffi';
import { globalEvents, RdeskEvent } from '../model/RdeskEvents';
import { routerParams, drainPendingChats } from '../model/Utils';

const LOG_DOMAIN = 0x5253;
const TAG = 'RustDeskChat';

class ChatMsg {
  text: string = '';
  // true = sent by the remote peer, false = sent by this device.
  fromPeer: boolean = false;
}

@Entry
@Component
struct ChatPage {
  @State connId: number = -1;
  @State peerName: string = '';
  @State messages: ChatMsg[] = [];
  @State draft: string = '';
  private scroller: Scroller = new Scroller();
  private handler = (ev: RdeskEvent): void => {
    this.onEvent(ev);
  };

  aboutToAppear(): void {
    const p = routerParams();
    this.connId = evNumParam(p['client']);
    this.peerName = evStrParam(p['name']);
    // The message that opened this page was dispatched before the
    // subscription below started; EntryAbility forwards it in the params.
    const firstMessage = evStrParam(p['firstMessage']);
    if (firstMessage.length > 0) {
      const msg = new ChatMsg();
      msg.text = firstMessage;
      msg.fromPeer = true;
      this.messages.push(msg);
    }
    for (const text of drainPendingChats(this.connId)) {
      const msg = new ChatMsg();
      msg.text = text;
      msg.fromPeer = true;
      this.messages.push(msg);
    }
    globalEvents.subscribe(this.handler);
  }

  aboutToDisappear(): void {
    globalEvents.unsubscribe(this.handler);
  }

  private onEvent(ev: RdeskEvent): void {
    if (ev.name !== 'cm_chat' || ev.num('id') !== this.connId) {
      return;
    }
    const msg = new ChatMsg();
    msg.text = ev.get('text');
    msg.fromPeer = true;
    this.messages.push(msg);
  }

  private send(): void {
    const text = this.draft.trim();
    if (text.length === 0) {
      return;
    }
    if (this.connId < 0) {
      promptAction.showToast({ message: $r('app.string.chat_no_connection'), duration: 2000 });
      return;
    }
    const rc = ffiServerSendChat(this.connId, text);
    if (rc !== 0) {
      hilog.warn(LOG_DOMAIN, TAG, 'send chat -> %{public}d', rc);
      // Keep the draft so the user can retry; nothing is appended.
      promptAction.showToast({ message: $r('app.string.chat_send_failed'), duration: 2000 });
      return;
    }
    const msg = new ChatMsg();
    msg.text = text;
    msg.fromPeer = false;
    this.messages.push(msg);
    this.draft = '';
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
        Text(this.peerName.length > 0 ? this.peerName : `#${this.connId}`)
          .fontSize(18)
          .fontWeight(FontWeight.Bold)
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .constraintSize({ maxWidth: '60%' })
        Blank()
        Text('')
          .fontSize(16)
      }
      .width('100%')
      .padding({ left: 16, right: 16, top: 16, bottom: 8 })

      List({ scroller: this.scroller, space: 8 }) {
        ForEach(this.messages, (m: ChatMsg, idx: number) => {
          ListItem() {
            Row() {
              if (m.fromPeer) {
                Text(m.text)
                  .fontSize(15)
                  .padding({ left: 12, right: 12, top: 8, bottom: 8 })
                  .borderRadius(12)
                  .constraintSize({ maxWidth: '78%' })
                  .backgroundColor('#E8F0FE')
                  .fontColor('#1A1A1A')
                Blank().layoutWeight(1)
              } else {
                Blank().layoutWeight(1)
                Text(m.text)
                  .fontSize(15)
                  .padding({ left: 12, right: 12, top: 8, bottom: 8 })
                  .borderRadius(12)
                  .constraintSize({ maxWidth: '78%' })
                  .backgroundColor('#1A6AF5')
                  .fontColor('#FFFFFF')
              }
            }
            .width('100%')
          }
          .margin({ top: idx === 0 ? 0 : 4 })
        }, (m: ChatMsg, idx: number) => `${idx}-${m.fromPeer}-${m.text}`)
      }
      .layoutWeight(1)
      .width('100%')
      .padding({ left: 16, right: 16, top: 8 })
      .scrollBar(BarState.Off)
      .onScrollIndex((start: number) => {
        // Anchor to the newest message while at the bottom.
        if (start >= this.messages.length - 1) {
          this.scroller.scrollToIndex(this.messages.length - 1, true);
        }
      })

      Row({ space: 8 }) {
        TextInput({ text: this.draft, placeholder: $r('app.string.chat_placeholder') })
          .layoutWeight(1)
          .height(42)
          .fontSize(15)
          .onChange((v: string) => {
            this.draft = v;
          })
          .onSubmit(() => {
            this.send();
          })
        Button($r('app.string.chat_send'))
          .height(42)
          .fontSize(15)
          .backgroundColor('#1A6AF5')
          .enabled(this.draft.trim().length > 0)
          .onClick(() => {
            this.send();
          })
      }
      .width('100%')
      .padding({ left: 16, right: 16, top: 8, bottom: 16 })
    }
    .width('100%')
    .height('100%')
    .backgroundColor('#FFFFFF')
  }
}

function evNumParam(v: Object | undefined | null): number {
  if (v === undefined || v === null) {
    return -1;
  }
  const n = parseFloat(`${v}`);
  return isNaN(n) ? -1 : n;
}

function evStrParam(v: Object | undefined | null): string {
  return v === undefined || v === null ? '' : `${v}`;
}
