import { router, promptAction, window } from '@kit.ArkUI';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { abilityAccessCtrl, PermissionRequestResult, Permissions } from '@kit.AbilityKit';
import { image } from '@kit.ImageKit';
import { pasteboard } from '@kit.BasicServicesKit';
import { util } from '@kit.ArkTS';
import { ffiSendMouse, ffiInputKey, ffiInputString, ffiInitVideo, ffiRefreshVideo,
         ffiSwitchDisplay, ffiSessionLogin, ffiSessionSend2fa, ffiSessionReconnect,
         ffiSessionClose, ffiSendClipboard, ffiRecordScreen, ffiRequestVoiceCall,
         ffiCloseVoiceCall, ffiAnswerVoiceCall, ffiSessionGetOption, ffiSessionSetOption,
         ffiSaveImageQuality, ffiSaveCustomImageQuality, ffiSetCustomFps, ffiToggleOption,
         ffiGetToggleOption, ffiUpdateSupportedDecodings, ffiAlternativeCodecs } from '../ohos_ffi';
import { globalEvents, RdeskEvent } from '../model/RdeskEvents';
import { evStr, evNum, evBool, parseJsonArray, routerParams } from '../model/Utils';

const LOG_DOMAIN = 0x5253;
const TAG = 'RustDeskRemote';

// Mouse mask encoding (contract §2, src/common.rs::input):
// mask = (buttons << 3) | type; type: MOVE=0 DOWN=1 UP=2 WHEEL=3.
const MOUSE_TYPE_MOVE = 0;
const MOUSE_TYPE_DOWN = 1;
const MOUSE_TYPE_UP = 2;
const MOUSE_TYPE_WHEEL = 3;
const MOUSE_BUTTON_LEFT = 0x01;
const MOUSE_BUTTON_RIGHT = 0x02;
const MOUSE_BUTTON_WHEEL = 0x04;
// Two-finger scroll sensitivity multiplier (tunable).
const WHEEL_FACTOR = 5;
const LONG_PRESS_MS = 500;
// Window after a tap in which starting a drag presses the left button
// (hold-drag, mirrors the Flutter client's mouse mode).
const DRAG_AFTER_TAP_MS = 350;
// Idle delay before the zoomed-pan scrollbar indicators fade.
const HIDE_DELAY_MS = 1500;
// Viewport zoom clamp: 1 = fit-to-screen, up to 8x magnification.
const MAX_ZOOM = 8;
// Multi-finger tap detection (onTouch): a touch counts as a tap when every
// finger stays within TAP_SLOP of its down point until all lift.
const TAP_SLOP = 15;
// Pointer (trackpad) mode: finger travel -> cursor travel ratio.
const POINTER_SPEED = 1.4;

// Image quality / fps slider bounds (flutter/lib/consts.dart).
const MIN_QUALITY = 10;
const MAX_QUALITY = 100;
const DEFAULT_QUALITY = 50;
const MIN_FPS = 5;
const MAX_FPS = 120;
const DEFAULT_FPS = 30;

// Display-settings radio entries: value understood by the peer option and the
// label resource. A class because ArkTS requires concrete types for literals.
class RadioItem {
  value: string = '';
  label: ResourceStr = '';
  enabled: boolean = true;
}

// Codec-preference alternative-codec flags, from ffiAlternativeCodecs.
class AlternativeCodecs {
  vp8: boolean = false;
  av1: boolean = false;
  h264: boolean = false;
  h265: boolean = false;
}

class DisplayItem {
  x: number = 0;
  y: number = 0;
  width: number = 0;
  height: number = 0;
  cursorEmbedded: boolean = false;
}

// One remote cursor shape, decoded from a cursor_data event. PixelMap is
// created asynchronously, so a shape may sit in the cache briefly without it.
class CursorShape {
  width: number = 0;
  height: number = 0;
  hotx: number = 0;
  hoty: number = 0;
  pm: image.PixelMap | undefined = undefined;
}

// One finger tracked by the multi-finger tap detector: down point (component
// local vp) and whether it has moved beyond the slop radius.
class TouchTrack {
  id: number = -1;
  startX: number = 0;
  startY: number = 0;
  moved: boolean = false;
}

// Quick-actions panel entry: label resource and the callback that runs when
// it is tapped. A class (not an object literal) because ArkTS requires a
// concrete type for every literal field, including callbacks.
class QuickAction {
  label: ResourceStr = '';
  onTap: () => void = (): void => {};
}

// Thin non-interactive scrollbar thumb drawn over the video container; a
// custom component because ArkUI's Scroll has no indicator-only mode.
@Component
struct ScrollbarIndicator {
  trackLength: number = 0;
  thumbLength: number = 0;
  thumbOffset: number = 0;
  vertical: boolean = true;

  build() {
    Stack({ alignContent: this.vertical ? Alignment.Center : Alignment.Center }) {
      Row()
        .backgroundColor('rgba(255,255,255,0.08)')
        .borderRadius(2)
        .width(this.vertical ? 4 : this.trackLength)
        .height(this.vertical ? this.trackLength : 4)
      Row()
        .backgroundColor('rgba(255,255,255,0.55)')
        .borderRadius(2)
        .width(this.vertical ? 4 : Math.min(this.thumbLength, this.trackLength))
        .height(this.vertical ? Math.min(this.thumbLength, this.trackLength) : 4)
        .margin({ top: this.vertical ? this.thumbOffset : 0,
                  left: this.vertical ? 0 : this.thumbOffset })
    }
    .width(this.vertical ? 8 : this.trackLength)
    .height(this.vertical ? this.trackLength : 8)
    .margin({ right: this.vertical ? 2 : 0, bottom: this.vertical ? 0 : 2 })
  }
}

// Toolbar icon item (sunlogin style): symbol glyph over a caption, active
// state tinted with the sunlogin blue. danger renders the caption red.
// A custom component, not a @Builder: by-value @Builder params never
// re-render when the backing @State changes, so the mode toggle's icon and
// the voice-call/record states would go stale while the panel stays open.
// @Prop syncs from the parent.
@Component
struct ToolbarItem {
  @Prop icon: Resource = $r('sys.symbol.keyboard');
  @Prop label: ResourceStr = '';
  @Prop on: boolean = false;
  @Prop danger: boolean = false;
  onTap: () => void = (): void => {};

  build() {
    Column({ space: 5 }) {
      SymbolGlyph(this.icon)
        .fontSize(24)
        .fontColor([this.danger ? '#FF6B6B' : (this.on ? '#4D8DFF' : '#E7EBF2')])
      Text(this.label)
        .fontSize(11)
        .fontColor(this.danger ? '#FF6B6B' : (this.on ? '#4D8DFF' : '#C3CAD6'))
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
    }
    .layoutWeight(1)
    .padding({ top: 8, bottom: 8 })
    .borderRadius(12)
    .backgroundColor(this.on ? 'rgba(26,106,245,0.18)' : Color.Transparent)
    .onClick(() => {
      this.onTap();
    })
  }
}

// One stat cell of the expanded status card: value over caption, green when
// the metric is healthy. A component for the same @Prop reason as above:
// speed/fps/delay update while the card is open.
@Component
struct StatCell {
  @Prop caption: ResourceStr = '';
  @Prop value: string = '';
  @Prop good: boolean = false;

  build() {
    Column({ space: 2 }) {
      Text(this.value.length > 0 ? this.value : '-')
        .fontSize(13)
        .fontWeight(FontWeight.Medium)
        .fontColor(this.good ? '#34C759' : '#FFB74D')
        .maxLines(1)
      Text(this.caption)
        .fontSize(10)
        .fontColor('#8A93A0')
    }
    .layoutWeight(1)
    .alignItems(HorizontalAlign.Start)
  }
}

@Entry
@Component
struct RemotePage {
  @State uuid: string = '';
  @State peerId: string = '';
  @State connState: string = 'connecting';
  @State connDetail: string = '';
  @State showStatusDetail: boolean = false;
  @State speed: string = '';
  @State fps: string = '';
  @State delay: string = '';
  @State peerName: string = '';
  @State platform: string = '';
  @State permissionText: string = '';
  @State displays: DisplayItem[] = [];
  @State currentDisplay: number = 0;
  @State remoteW: number = 0;
  @State remoteH: number = 0;
  @State videoW: number = 0;
  @State videoH: number = 0;
  @State containerW: number = 0;
  @State containerH: number = 0;
  // Viewport transform: pinch zoom (scale about the fit-size center) plus
  // two-finger pan offsets, both in vp relative to the letterbox center.
  @State viewScale: number = 1;
  @State viewX: number = 0;
  @State viewY: number = 0;
  // Landscape goes fullscreen: system bars are hidden by EntryAbility, the
  // toolbar auto-hides and floats back via the corner chip.
  @State isLandscape: boolean = false;
  @State landscapeToolbar: boolean = false;
  // msgbox dialogs
  @State showPasswordDialog: boolean = false;
  @State showTwofaDialog: boolean = false;
  @State showErrorDialog: boolean = false;
  @State msgboxText: string = '';
  @State msgboxHasRetry: boolean = false;
  @State passwordInput: string = '';
  @State rememberPassword: boolean = false;
  @State twofaInput: string = '';
  @State trustDevice: boolean = false;
  // toolbar / soft keyboard
  @State showKeyboard: boolean = false;
  @State ctrlOn: boolean = false;
  @State altOn: boolean = false;
  @State shiftOn: boolean = false;
  @State keyboardInput: string = '';
  private sentKeyboardText: string = '';
  // gesture bookkeeping
  private lastWheelOffsetY: number = 0;
  private lastWheelOffsetX: number = 0;
  private pinchBase: number = 1;
  private pinching: boolean = false;
  private lastFocalX: number = 0;
  private lastFocalY: number = 0;
  // One-finger drag: moves the cursor only; a drag that starts right after a
  // tap (hold-drag) presses the left button instead.
  private panDragging: boolean = false;
  private lastTapMs: number = 0;
  // Multi-finger tap detection on raw touches (TapGesture never fired for
  // injected touches in E2E testing, so two/three-finger taps are detected
  // manually here): tracked fingers, their count at lift time and whether
  // the whole gesture was still a tap.
  private touchTracks: TouchTrack[] = [];
  private tapFingerCount: number = 0;
  private tapValid: boolean = false;
  // Down-midpoint of the current multi-finger gesture (component-local vp).
  private tapMidX: number = 0;
  private tapMidY: number = 0;
  // Input mode: touch (default, absolute) vs pointer (relative trackpad).
  @State pointerMode: boolean = false;
  // Virtual pointer position in video-local vp (pointer mode only); the
  // single finger is a trackpad moving this cursor, clicks land on it.
  @State vptrX: number = 0;
  @State vptrY: number = 0;
  // Left button is held while pointer-mode dragging.
  private vptrDragging: boolean = false;
  // Long-press in pointer mode holds the right button until the finger lifts.
  private vptrRightHeld: boolean = false;
  // Previous finger position (component-local vp); pointer-mode deltas are
  // measured against it.
  private lastPtrX: number = 0;
  private lastPtrY: number = 0;
  // Quick-actions panel (three-finger tap / menu entry).
  @State showQuickActions: boolean = false;
  // Virtual-mouse pad (pointer mode): left/right click, double click, wheel.
  @State showVirtualMouse: boolean = false;
  // Sunlogin-style bottom toolbar: a corner chip toggles an icon panel that
  // holds every toolbar function (replaces the floating ball + menu drawer).
  @State showToolbar: boolean = false;
  // Display settings panel: image quality / codec preference / view-only.
  @State showDisplaySettings: boolean = false;
  // Toolbar chip position: right-edge/bottom coordinates (markAnchor 100%).
  // Negative = not seeded yet; the root onAreaChange initializes it.
  @State ballX: number = -1;
  @State ballY: number = -1;
  @State rootW: number = 0;
  @State rootH: number = 0;
  private ballDragFromX: number = 0;
  private ballDragFromY: number = 0;
  // "low" | "balanced" | "best" | "custom"; the slider shows for "custom".
  @State imageQuality: string = 'balanced';
  @State customQuality: number = DEFAULT_QUALITY;
  @State customFps: number = DEFAULT_FPS;
  // Selected codec-preference value ("auto"/"vp8"/"vp9"/"h264"/"h265").
  @State codecPreference: string = 'auto';
  @State altCodecs: AlternativeCodecs = new AlternativeCodecs();
  @State viewOnly: boolean = false;
  // Voice call state machine: idle -> waiting (peer deciding) -> active.
  @State voiceCallState: string = 'idle';
  // The controlled side is offering a voice call and awaits our answer.
  @State showVoiceCallIncomingDialog: boolean = false;
  // Screen recording toggle (follows the peer's recording permission).
  @State recording: boolean = false;
  // Recording allowed by the peer (permission event "recording").
  @State recordingAllowed: boolean = false;
  // Audio permission from the peer ("audio" permission event); when false the
  // audio chip shows disabled state.
  @State audioAllowed: boolean = false;
  // Scrollbar indicators while zoomed and panning.
  @State showScrollbars: boolean = false;
  private scrollbarTimer: number = -1;
  // Remote cursor (the peer's real pointer, distinct from the touch overlay):
  // position in remote pixels, current shape as @State fields mirroring the
  // entry in the shape cache. Mirrors Flutter mobile's showCursorPaint (drawn
  // unless the peer is Android or embeds the cursor in the video).
  @State showRemoteCursor: boolean = true;
  @State rcurX: number = 0;
  @State rcurY: number = 0;
  @State rcurPm: image.PixelMap | undefined = undefined;
  @State rcurW: number = 0;
  @State rcurH: number = 0;
  @State rcurHotX: number = 0;
  @State rcurHotY: number = 0;
  @State cursorEmbedded: boolean = false;
  private cursorShapes: Map<string, CursorShape> = new Map();
  // Shape the overlay is pointing at; guards the async decode against
  // re-selecting a stale shape after the peer switched to another one.
  private rcurId: string = '';
  private xcController: XComponentController = new XComponentController();
  private eventHandler: ((ev: RdeskEvent) => void) | null = null;

  aboutToAppear(): void {
    const p = routerParams();
    this.uuid = evStr(p['uuid']);
    this.peerId = evStr(p['peerId']);
    this.eventHandler = (ev: RdeskEvent): void => {
      this.handleEvent(ev);
    };
    globalEvents.subscribe(this.eventHandler);
    this.setKeepScreenOn(true);
  }

  aboutToDisappear(): void {
    if (this.eventHandler !== null) {
      globalEvents.unsubscribe(this.eventHandler);
      this.eventHandler = null;
    }
    this.setKeepScreenOn(false);
    // Idempotent: Rust returns -1 for an unknown/already-closed session.
    ffiSessionClose(this.uuid);
  }

  // A remote-control session is a watching experience: keep the screen from
  // sleeping while the page is on top.
  private setKeepScreenOn(on: boolean): void {
    window.getLastWindow(getContext(this)).then((win: window.Window) => {
      return win.setWindowKeepScreenOn(on);
    }).catch((e: BusinessError) => {
      hilog.warn(LOG_DOMAIN, TAG, 'setWindowKeepScreenOn(%{public}s) failed: %{public}s',
                 `${on}`, `${e}`);
    });
  }

  // ------------------------------------------------------------------
  // Event handling (JS thread, via threadsafe function)
  // ------------------------------------------------------------------

  private handleEvent(ev: RdeskEvent): void {
    switch (ev.name) {
      case 'peer_info':
        this.onPeerInfo(ev);
        break;
      case 'sync_peer_info':
        this.onDisplaysJson(ev.get('displays'), -1);
        break;
      case 'switch_display':
        this.currentDisplay = ev.num('display');
        this.remoteW = ev.num('width');
        this.remoteH = ev.num('height');
        this.cursorEmbedded = ev.bool('cursor_embedded');
        this.fitVideo();
        break;
      case 'connection_ready':
        this.connState = 'ready';
        this.connDetail = (ev.bool('secure') ? 'encrypted' : 'unencrypted') +
          (ev.bool('direct') ? ', direct' : ', relayed');
        break;
      case 'cursor_data':
        this.onCursorData(ev);
        break;
      case 'cursor_id':
        this.applyCursorShape(ev.get('id'));
        break;
      case 'cursor_position':
        this.rcurX = ev.num('x');
        this.rcurY = ev.num('y');
        break;
      case 'msgbox':
        this.onMsgbox(ev);
        break;
      case 'cancel_msgbox':
        this.showPasswordDialog = false;
        this.showTwofaDialog = false;
        this.showErrorDialog = false;
        this.showVoiceCallIncomingDialog = false;
        break;
      case 'permission': {
        const parts: string[] = [];
        for (const k of ev.keys) {
          parts.push(`${k}: ${ev.get(k) === 'true' ? 'Y' : 'N'}`);
          if (k === 'recording') {
            this.recordingAllowed = ev.get(k) === 'true';
            if (!this.recordingAllowed) {
              this.recording = false;
            }
          } else if (k === 'audio') {
            this.audioAllowed = ev.get(k) === 'true';
          }
        }
        this.permissionText = parts.join('  ');
        break;
      }
      case 'update_quality_status':
        this.speed = ev.get('speed');
        this.fps = ev.get('fps');
        this.delay = ev.get('delay');
        break;
      case 'voice_call_waiting':
        this.voiceCallState = 'waiting';
        break;
      case 'voice_call_started':
        this.voiceCallState = 'active';
        this.showVoiceCallIncomingDialog = false;
        break;
      case 'voice_call_closed':
        this.voiceCallState = 'idle';
        this.showVoiceCallIncomingDialog = false;
        break;
      case 'voice_call_incoming':
        // The controlled side offers a voice call; the dialog answers it.
        this.showVoiceCallIncomingDialog = true;
        break;
      case 'record_status':
        this.recording = ev.get('start') === 'true';
        break;
      case 'clipboard_image':
        this.onClipboardImage(ev.get('png'));
        break;
      case 'clipboard_html':
        this.onClipboardHtml(ev.get('html'));
        break;
      default:
        break;
    }
  }

  private onPeerInfo(ev: RdeskEvent): void {
    // PeerInfo only arrives after the peer accepted the login request, so any
    // pending credential prompt is stale at this point (mirrors Flutter's
    // dialogManager.dismissAll() in handlePeerInfo).
    this.showPasswordDialog = false;
    this.showTwofaDialog = false;
    this.peerName = ev.get('username');
    this.platform = ev.get('platform');
    if (this.peerName.length > 0) {
      this.peerName = `${this.peerName}@${ev.get('hostname')}`;
    }
    this.onDisplaysJson(ev.get('displays'), ev.num('current_display'));
  }

  private onDisplaysJson(json: string, current: number): void {
    const list: DisplayItem[] = [];
    const arr = parseJsonArray(json);
    for (const it of arr) {
      const d = new DisplayItem();
      d.x = evNum(it['x']);
      d.y = evNum(it['y']);
      d.width = evNum(it['width']);
      d.height = evNum(it['height']);
      d.cursorEmbedded = evBool(it['cursor_embedded']);
      list.push(d);
    }
    this.displays = list;
    if (current >= 0 && current < list.length) {
      this.currentDisplay = current;
    }
    if (this.currentDisplay >= 0 && this.currentDisplay < list.length) {
      this.remoteW = list[this.currentDisplay].width;
      this.remoteH = list[this.currentDisplay].height;
      this.cursorEmbedded = list[this.currentDisplay].cursorEmbedded;
    }
    this.fitVideo();
  }

  // ------------------------------------------------------------------
  // Remote cursor
  // ------------------------------------------------------------------

  // A new shape (or a re-send of one not decoded yet): cache it and select it.
  // Position updates ride the separate cursor_position events.
  private onCursorData(ev: RdeskEvent): void {
    const id = ev.get('id');
    const w = ev.num('width');
    const h = ev.num('height');
    const need = w * h * 4;
    if (id.length === 0 || w <= 0 || h <= 0) {
      return;
    }
    if (!this.cursorShapes.has(id)) {
      const shape = new CursorShape();
      shape.width = w;
      shape.height = h;
      shape.hotx = ev.num('hotx');
      shape.hoty = ev.num('hoty');
      this.cursorShapes.set(id, shape);
      let bytes: Uint8Array = new Uint8Array(0);
      try {
        bytes = new util.Base64Helper().decodeSync(ev.get('colors'));
      } catch (e) {
        hilog.warn(LOG_DOMAIN, TAG, 'cursor colors decode failed: %{public}s', `${e}`);
      }
      if (bytes.byteLength < need) {
        this.cursorShapes.delete(id);
        return;
      }
      const buf = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + need);
      const opts: image.InitializationOptions = {
        size: { width: w, height: h },
        pixelFormat: image.PixelMapFormat.RGBA_8888,
      };
      image.createPixelMap(buf, opts).then((pm: image.PixelMap) => {
        const shape2 = this.cursorShapes.get(id);
        if (shape2 !== undefined) {
          shape2.pm = pm;
          // Only re-apply if no other shape was selected while decoding.
          if (this.rcurId === id) {
            this.applyCursorShape(id);
          }
        } else {
          pm.release();
        }
      }).catch((e: BusinessError) => {
        hilog.warn(LOG_DOMAIN, TAG, 'createPixelMap failed: %{public}s', `${e}`);
        this.cursorShapes.delete(id);
      });
    }
    this.applyCursorShape(id);
  }

  // Point the overlay at a cached shape; keeps the last one when unknown (the
  // peer only re-sends a shape after it changed or on reconnect).
  private applyCursorShape(id: string): void {
    this.rcurId = id;
    const shape = this.cursorShapes.get(id);
    if (shape === undefined) {
      return;
    }
    if (shape.pm !== undefined) {
      this.rcurPm = shape.pm;
      this.rcurW = shape.width;
      this.rcurH = shape.height;
      this.rcurHotX = shape.hotx;
      this.rcurHotY = shape.hoty;
    }
  }

  // Display factor: remote pixels -> video-local vp (the wrapper's zoom
  // transform scales both video and cursor together).
  private cursorScale(): number {
    if (this.remoteW <= 0 || this.videoW <= 0) {
      return 1;
    }
    return this.videoW / this.remoteW;
  }

  private remoteCursorDrawn(): boolean {
    return this.showRemoteCursor && !this.cursorEmbedded &&
      this.platform !== 'Android' && this.rcurPm !== undefined &&
      this.remoteW > 0 && this.remoteH > 0;
  }

  private onMsgbox(ev: RdeskEvent): void {
    const type = ev.get('type');
    this.msgboxText = ev.get('text');
    this.msgboxHasRetry = ev.bool('hasRetry');
    if (type === 'input-password' || type === 're-input-password') {
      this.showPasswordDialog = true;
    } else if (type === 'input-2fa') {
      this.showTwofaDialog = true;
    } else if (this.msgboxHasRetry) {
      this.showErrorDialog = true;
    } else {
      promptAction.showToast({ message: this.msgboxText, duration: 3000 });
    }
  }

  // ------------------------------------------------------------------
  // Input
  // ------------------------------------------------------------------

  private mouse(mask: number, x: number, y: number): void {
    ffiSendMouse(this.uuid, mask, x, y, this.altOn, this.ctrlOn, this.shiftOn, false);
  }

  // The peer expects absolute coordinates in the virtual-desktop space, so
  // the selected display's origin is added on top of the mapped in-display
  // position (wheel events carry no position and skip this).
  private displayOriginX(): number {
    const d = this.displays[this.currentDisplay];
    return d ? d.x : 0;
  }

  private displayOriginY(): number {
    const d = this.displays[this.currentDisplay];
    return d ? d.y : 0;
  }

  private mapX(localX: number): number {
    if (this.videoW <= 0 || this.remoteW <= 0) {
      return 0;
    }
    let v = Math.floor(localX / this.videoW * this.remoteW);
    if (v < 0) {
      v = 0;
    }
    if (v > this.remoteW - 1) {
      v = this.remoteW - 1;
    }
    return v;
  }

  private mapY(localY: number): number {
    if (this.videoH <= 0 || this.remoteH <= 0) {
      return 0;
    }
    let v = Math.floor(localY / this.videoH * this.remoteH);
    if (v < 0) {
      v = 0;
    }
    if (v > this.remoteH - 1) {
      v = this.remoteH - 1;
    }
    return v;
  }

  private pressKey(name: string): void {
    ffiInputKey(this.uuid, name, false, true, this.altOn, this.ctrlOn, this.shiftOn, false);
  }

  // Soft keyboard diff: send only the appended tail; local deletions become
  // VK_BACK clicks.
  private onKeyboardChange(value: string): void {
    const sent = this.sentKeyboardText;
    if (value === sent) {
      return;
    }
    let common = 0;
    while (common < sent.length && common < value.length &&
           sent.charAt(common) === value.charAt(common)) {
      common++;
    }
    const deletions = sent.length - common;
    for (let i = 0; i < deletions; i++) {
      this.pressKey('VK_BACK');
    }
    if (common < value.length) {
      ffiInputString(this.uuid, value.substring(common));
    }
    this.sentKeyboardText = value;
  }

  // Touch position relative to the video component, in vp. GestureEvent carries
  // no x/y in this SDK; FingerInfo.localX/localY is the equivalent. Slots can
  // be undefined for injected/multi-finger touches (a two-finger touch also
  // activates the one-finger recognizers), so scan for a real entry - reading
  // .localX off an undefined slot crashes the app.
  private fingerX(e: GestureEvent): number {
    for (const f of e.fingerList) {
      if (f !== undefined && f !== null) {
        return f.localX;
      }
    }
    return 0;
  }

  private fingerY(e: GestureEvent): number {
    for (const f of e.fingerList) {
      if (f !== undefined && f !== null) {
        return f.localY;
      }
    }
    return 0;
  }

  // Mean finger position, in component-local vp. The pinch gesture uses it as
  // its focal point so zooming also follows finger drift (standard viewer
  // behavior); a pure two-finger drag keeps the inter-finger distance, but the
  // pinch recognizer still fires first in practice, so this - not the
  // two-finger pan - is what pans the zoomed viewport.
  private focalX(e: GestureEvent): number {
    let sum = 0;
    let count = 0;
    for (const f of e.fingerList) {
      if (f !== undefined && f !== null) {
        sum += f.localX;
        count += 1;
      }
    }
    return count > 0 ? sum / count : 0;
  }

  private focalY(e: GestureEvent): number {
    let sum = 0;
    let count = 0;
    for (const f of e.fingerList) {
      if (f !== undefined && f !== null) {
        sum += f.localY;
        count += 1;
      }
    }
    return count > 0 ? sum / count : 0;
  }

  // ------------------------------------------------------------------
  // Multi-finger tap detection (onTouch)
  // ------------------------------------------------------------------

  // Touches feed two finger-count groups: taps of >= 2 fingers must not
  // interfere with the single-finger pan/tap path, and the tap recognizers
  // do not fire for injected touches, so two-/three-finger taps are detected
  // from raw TouchEvents: a gesture is an N-finger tap when N fingers were
  // down together and every one of them lifted within the slop radius.
  private onTouchEvent(e: TouchEvent): void {
    const changed: TouchObject[] = [];
    for (const t of e.changedTouches) {
      if (t !== undefined && t !== null) {
        changed.push(t);
      }
    }
    if (e.type === TouchType.Down) {
      for (const t of changed) {
        const track = new TouchTrack();
        track.id = t.id;
        track.startX = t.x;
        track.startY = t.y;
        this.touchTracks.push(track);
      }
      // A second/third finger joining while a one-finger pan already sent
      // moves invalidates the whole gesture as a tap (the user is dragging
      // or scrolling, not tapping).
      if (this.touchTracks.length >= 2 &&
          (this.panDragging || this.vptrDragging)) {
        this.tapValid = false;
        return;
      }
      this.tapFingerCount = this.touchTracks.length;
      this.tapValid = this.touchTracks.length >= 2;
      // Midpoint of all down points: where a two-finger right-click lands,
      // independent of which finger lifts first.
      let mx = 0;
      let my = 0;
      for (const track of this.touchTracks) {
        mx += track.startX;
        my += track.startY;
      }
      this.tapMidX = mx / this.touchTracks.length;
      this.tapMidY = my / this.touchTracks.length;
      return;
    }
    if (e.type === TouchType.Move) {
      for (const t of changed) {
        for (const track of this.touchTracks) {
          if (track.id === t.id) {
            const dx = t.x - track.startX;
            const dy = t.y - track.startY;
            if (dx * dx + dy * dy > TAP_SLOP * TAP_SLOP) {
              track.moved = true;
            }
          }
        }
      }
      return;
    }
    if (e.type !== TouchType.Up && e.type !== TouchType.Cancel) {
      return;
    }
    // Up / Cancel: invalidate when a lifted finger had moved, then forget it.
    for (const t of changed) {
      for (const track of this.touchTracks) {
        if (track.id === t.id && track.moved) {
          this.tapValid = false;
        }
      }
      this.touchTracks = this.touchTracks.filter((tr: TouchTrack): boolean => tr.id !== t.id);
    }
    // Pointer-mode right-hold releases on the raw finger lift: after a
    // long-press wins the exclusive group the pan recognizer never fires, so
    // onActionEnd cannot be the release point.
    if (this.pointerMode && this.vptrRightHeld && this.touchTracks.length === 0) {
      this.vptrMouse((MOUSE_BUTTON_RIGHT << 3) | MOUSE_TYPE_UP);
      this.vptrRightHeld = false;
    }
    if (this.touchTracks.length > 0) {
      return;
    }
    const fingers = this.tapFingerCount;
    const valid = this.tapValid;
    this.tapFingerCount = 0;
    this.tapValid = false;
    if (!valid || e.type === TouchType.Cancel) {
      return;
    }
    if (fingers === 2) {
      this.onTwoFingerTap();
    } else if (fingers === 3) {
      this.onThreeFingerTap();
    }
  }

  // RD Client-style right click: both fingers land and lift together, the
  // click lands on their down-midpoint.
  private onTwoFingerTap(): void {
    if (this.pointerMode) {
      this.vptrMouse((MOUSE_BUTTON_RIGHT << 3) | MOUSE_TYPE_DOWN);
      this.vptrMouse((MOUSE_BUTTON_RIGHT << 3) | MOUSE_TYPE_UP);
      return;
    }
    this.mouseAt((MOUSE_BUTTON_RIGHT << 3) | MOUSE_TYPE_DOWN, this.tapMidX, this.tapMidY);
    this.mouseAt((MOUSE_BUTTON_RIGHT << 3) | MOUSE_TYPE_UP, this.tapMidX, this.tapMidY);
  }

  // Quick-actions panel (task manager, Ctrl+Alt+Del, Alt+F4, ...).
  private onThreeFingerTap(): void {
    this.showQuickActions = true;
  }

  // ------------------------------------------------------------------
  // Pointer (trackpad) mode
  // ------------------------------------------------------------------

  // The virtual cursor lives in video-local vp and is sent to the remote in
  // remote pixels like any absolute click; clamped to the video so the
  // trackpad can never lose the cursor off-screen.
  private vptrClamp(): void {
    if (this.vptrX < 0) {
      this.vptrX = 0;
    }
    if (this.vptrY < 0) {
      this.vptrY = 0;
    }
    if (this.vptrX > this.videoW - 1) {
      this.vptrX = this.videoW - 1;
    }
    if (this.vptrY > this.videoH - 1) {
      this.vptrY = this.videoH - 1;
    }
  }

  // Send a mouse event at the virtual cursor position (pointer mode only).
  private vptrMouse(mask: number): void {
    this.mouse(mask, this.mapX(this.vptrX) + this.displayOriginX(),
      this.mapY(this.vptrY) + this.displayOriginY());
  }

  // Container-local draw position for the virtual cursor: vptrX/Y live in
  // video-local vp, the overlay Path is a container sibling, so apply the
  // video's centering offset and the zoom transform like the video wrapper
  // does (scale is about the video center).
  private vptrDrawX(): number {
    return this.videoLeft() +
      (this.vptrX - this.videoW / 2) * this.viewScale + this.videoW / 2;
  }

  private vptrDrawY(): number {
    return this.videoTop() +
      (this.vptrY - this.videoH / 2) * this.viewScale + this.videoH / 2;
  }

  private enterPointerMode(): void {
    if (this.pointerMode) {
      return;
    }
    // Enter with the cursor at the center of the video (or the last remote
    // cursor position when the peer reports one).
    this.vptrX = this.remoteCursorDrawn() ? this.rcurX * this.cursorScale() : this.videoW / 2;
    this.vptrY = this.remoteCursorDrawn() ? this.rcurY * this.cursorScale() : this.videoH / 2;
    this.vptrClamp();
    // The two modes are mutually exclusive: the virtual pointer replaces the
    // remote-cursor overlay while it is active.
    this.showRemoteCursor = false;
    this.pointerMode = true;
    this.showVirtualMouse = true;
  }

  private leavePointerMode(): void {
    if (!this.pointerMode) {
      return;
    }
    this.pointerMode = false;
    // Release any held button so switching modes mid-hold cannot leave the
    // remote with a stuck mouse button.
    if (this.vptrRightHeld) {
      this.vptrMouse((MOUSE_BUTTON_RIGHT << 3) | MOUSE_TYPE_UP);
      this.vptrRightHeld = false;
    }
    if (this.vptrDragging) {
      this.vptrMouse((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_UP);
      this.vptrDragging = false;
    }
    // Back to cursor mode: restore the remote-cursor overlay.
    this.showRemoteCursor = true;
    this.showVirtualMouse = false;
  }

  // Send a mouse event at a finger position (component-local vp). The
  // finger itself steers the pointer; no local arrow overlay is drawn.
  private mouseAt(mask: number, fingerX: number, fingerY: number): void {
    this.mouse(mask, this.mapX(fingerX) + this.displayOriginX(),
      this.mapY(fingerY) + this.displayOriginY());
  }

  // Flash the scrollbars while zoomed; called on any pan/zoom change.
  private pokeScrollbars(): void {
    if (this.viewScale <= 1) {
      this.showScrollbars = false;
      return;
    }
    this.showScrollbars = true;
    if (this.scrollbarTimer >= 0) {
      clearTimeout(this.scrollbarTimer);
    }
    this.scrollbarTimer = setTimeout((): void => {
      this.showScrollbars = false;
      this.scrollbarTimer = -1;
    }, HIDE_DELAY_MS) as number;
  }

  // Viewport position of the video's top-left corner in the container, in vp,
  // for mapping pan offsets onto scrollbar positions.
  private videoLeft(): number {
    return (this.containerW - this.videoW) / 2 + this.viewX;
  }

  private videoTop(): number {
    return (this.containerH - this.videoH) / 2 + this.viewY;
  }

  private disconnect(): void {
    ffiSessionClose(this.uuid);
    router.back();
  }

  // ------------------------------------------------------------------
  // Sunlogin-style bottom toolbar: corner chip + icon panel
  // ------------------------------------------------------------------

  // Seed the toolbar chip position once the root size is known: right edge
  // with a 10vp gap, 42% height (markAnchor 100% makes x the chip's right
  // edge). Kept draggable like the old floating ball.
  private seedBallPos(): void {
    if (this.ballX < 0 && this.rootW > 0 && this.rootH > 0) {
      this.ballX = this.rootW - 10;
      this.ballY = this.rootH * 0.42;
    }
  }

  private onBallDragStart(): void {
    this.ballDragFromX = this.ballX;
    this.ballDragFromY = this.ballY;
    this.showToolbar = false;
  }

  private onBallDragUpdate(dx: number, dy: number): void {
    if (this.rootW <= 0 || this.rootH <= 0) {
      return;
    }
    // markAnchor(100%,100%) means x/y track the ball's bottom-right corner;
    // clamp so a slice of the ball always stays grabbable on every edge.
    const x = Math.min(Math.max(dx, 30), this.rootW - 4);
    const y = Math.min(Math.max(dy, 40), this.rootH - 4);
    this.ballX = x;
    this.ballY = y;
  }

  // Ball quick action: cycle to the next display (single tap). With one
  // display it does nothing.
  private quickSwitchDisplay(): void {
    if (this.displays.length < 2) {
      promptAction.showToast({ message: $r('app.string.display'), duration: 1500 });
      return;
    }
    const next = (this.currentDisplay + 1) % this.displays.length;
    this.switchToDisplay(next);
  }

  private switchToDisplay(idx: number): void {
    if (idx < 0 || idx >= this.displays.length || idx === this.currentDisplay) {
      return;
    }
    ffiSwitchDisplay(this.uuid, idx);
  }

  private quickToggleMode(): void {
    if (this.pointerMode) {
      this.leavePointerMode();
      promptAction.showToast({ message: $r('app.string.mode_switched_touch'), duration: 1500 });
    } else {
      this.enterPointerMode();
      promptAction.showToast({ message: $r('app.string.mode_switched_pointer'), duration: 1500 });
    }
  }

  // ------------------------------------------------------------------
  // Virtual mouse pad (pointer mode): clickable buttons that act on the
  // virtual cursor position; wheel buttons scroll the remote content.
  // ------------------------------------------------------------------

  private vmLeftClick(): void {
    this.vptrMouse((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_DOWN);
    this.vptrMouse((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_UP);
  }

  private vmDoubleClick(): void {
    this.vmLeftClick();
    this.vmLeftClick();
  }

  private vmRightClick(): void {
    this.vptrMouse((MOUSE_BUTTON_RIGHT << 3) | MOUSE_TYPE_DOWN);
    this.vptrMouse((MOUSE_BUTTON_RIGHT << 3) | MOUSE_TYPE_UP);
  }

  private vmMiddleClick(): void {
    this.vptrMouse((MOUSE_BUTTON_WHEEL << 3) | MOUSE_TYPE_DOWN);
    this.vptrMouse((MOUSE_BUTTON_WHEEL << 3) | MOUSE_TYPE_UP);
  }

  // Wheel: y > 0 scrolls up on Windows peers (the desktop server multiplies
  // non-trackpad wheel deltas by WHEEL_DELTA), matching the two-finger
  // drag path that already sends raw positive/negative amounts.
  private vmWheel(up: boolean): void {
    const amount = 3;
    this.mouse((MOUSE_BUTTON_WHEEL << 3) | MOUSE_TYPE_WHEEL, 0, up ? amount : -amount);
  }

  private toggleVirtualMouse(): void {
    if (this.pointerMode) {
      this.showVirtualMouse = !this.showVirtualMouse;
    } else {
      // First tap enters pointer mode and opens the pad; the mode toast is
      // skipped here because the pad itself is the visible feedback.
      this.enterPointerMode();
    }
  }

  // Local -> remote clipboard. pasteboard.getData() needs the ACL-restricted
  // ohos.permission.READ_PASTEBOARD (not in the current debug profile), so the
  // content rides the system's own paste affordance instead: the user
  // long-presses the input and pastes, which needs no permission.
  @State showClipboardDialog: boolean = false;
  @State clipboardInput: string = '';

  private syncClipboard(): void {
    this.clipboardInput = '';
    this.showClipboardDialog = true;
  }

  private sendClipboardInput(): void {
    this.showClipboardDialog = false;
    const text = this.clipboardInput;
    this.clipboardInput = '';
    if (text.length === 0) {
      promptAction.showToast({ message: $r('app.string.clipboard_empty'), duration: 2000 });
      return;
    }
    const rc = ffiSendClipboard(this.uuid, text);
    if (rc === 0) {
      promptAction.showToast({ message: $r('app.string.clipboard_sent'), duration: 2000 });
    } else {
      promptAction.showToast({ message: $r('app.string.clipboard_send_failed'), duration: 2000 });
    }
  }

  // ------------------------------------------------------------------
  // Voice call / session recording
  // ------------------------------------------------------------------

  private toggleVoiceCall(): void {
    if (this.voiceCallState === 'idle') {
      this.requestMicThenCall();
    } else {
      ffiCloseVoiceCall(this.uuid);
      this.voiceCallState = 'idle';
    }
  }

  // ohos.permission.MICROPHONE is user_grant: the first voice call triggers
  // the system dialog; a denial keeps the call idle.
  private requestMicThenCall(): void {
    const perms: Array<Permissions> = ['ohos.permission.MICROPHONE'];
    abilityAccessCtrl.createAtManager().requestPermissionsFromUser(getContext(this), perms)
      .then((result: PermissionRequestResult) => {
        const granted = result.authResults.every((r: number): boolean => r === 0);
        if (!granted) {
          promptAction.showToast({ message: $r('app.string.mic_denied'), duration: 2500 });
          return;
        }
        this.voiceCallState = 'waiting';
        if (ffiRequestVoiceCall(this.uuid) !== 0) {
          this.voiceCallState = 'idle';
          promptAction.showToast({ message: $r('app.string.voice_call_failed'), duration: 2000 });
        }
      })
      .catch((e: BusinessError) => {
        hilog.warn(LOG_DOMAIN, TAG, 'request mic permission failed: %{public}s', `${e}`);
      });
  }

  // Answer a voice call the controlled side offered. Accepting requests the
  // mic permission first; a denial declines so the peer is not left waiting
  // on a one-way call.
  private answerVoiceCall(accept: boolean): void {
    this.showVoiceCallIncomingDialog = false;
    if (!accept) {
      ffiAnswerVoiceCall(this.uuid, false);
      return;
    }
    const perms: Array<Permissions> = ['ohos.permission.MICROPHONE'];
    abilityAccessCtrl.createAtManager().requestPermissionsFromUser(getContext(this), perms)
      .then((result: PermissionRequestResult) => {
        const granted = result.authResults.every((r: number): boolean => r === 0);
        if (!granted) {
          promptAction.showToast({ message: $r('app.string.mic_denied'), duration: 2500 });
          ffiAnswerVoiceCall(this.uuid, false);
          return;
        }
        ffiAnswerVoiceCall(this.uuid, true);
      })
      .catch((e: BusinessError) => {
        hilog.warn(LOG_DOMAIN, TAG, 'request mic permission failed: %{public}s', `${e}`);
        ffiAnswerVoiceCall(this.uuid, false);
      });
  }

  private toggleRecording(): void {
    if (!this.recordingAllowed) {
      promptAction.showToast({ message: $r('app.string.record_not_allowed'), duration: 2000 });
      return;
    }
    const start = !this.recording;
    if (ffiRecordScreen(this.uuid, start) === 0) {
      this.recording = start;
    }
  }

  // ------------------------------------------------------------------
  // Display settings: image quality / codec preference / view-only
  // ------------------------------------------------------------------

  // Load the current settings from the peer config when the panel opens; the
  // same keys the Flutter toolbar reads.
  private openDisplaySettings(): void {
    const q = ffiSessionGetOption(this.uuid, 'image_quality');
    if (q.length > 0) {
      this.imageQuality = q;
    } else {
      this.imageQuality = 'balanced';
    }
    const cq = ffiSessionGetOption(this.uuid, 'custom_image_quality');
    const cqNum = Number(cq);
    this.customQuality =
      cq.length > 0 && cqNum >= MIN_QUALITY && cqNum <= MAX_QUALITY * 20 ? cqNum : DEFAULT_QUALITY;
    const fps = ffiSessionGetOption(this.uuid, 'custom-fps');
    const fpsNum = Number(fps);
    this.customFps = fps.length > 0 && fpsNum >= MIN_FPS && fpsNum <= MAX_FPS ? fpsNum : DEFAULT_FPS;
    const cp = ffiSessionGetOption(this.uuid, 'codec-preference');
    this.codecPreference = cp.length > 0 ? cp : 'auto';
    this.viewOnly = ffiGetToggleOption(this.uuid, 'view-only') === 1;
    try {
      const codecs: Object = JSON.parse(ffiAlternativeCodecs(this.uuid)) as Object;
      const rec: Record<string, Object> = codecs as Record<string, Object>;
      const alt = new AlternativeCodecs();
      alt.vp8 = rec['vp8'] === true;
      alt.av1 = rec['av1'] === true;
      alt.h264 = rec['h264'] === true;
      alt.h265 = rec['h265'] === true;
      this.altCodecs = alt;
    } catch (e) {
      this.altCodecs = new AlternativeCodecs();
    }
    this.showDisplaySettings = true;
  }

  private pickImageQuality(value: string): void {
    this.imageQuality = value;
    ffiSaveImageQuality(this.uuid, value);
  }

  // Slider moved (debounce-free: the FFI round-trip is in-process; the peer
  // applies OptionMessage throttling itself).
  private applyCustomQuality(quality: number): void {
    this.customQuality = quality;
    this.imageQuality = 'custom';
    ffiSaveCustomImageQuality(this.uuid, Math.round(quality));
  }

  private applyCustomFps(fps: number): void {
    this.customFps = fps;
    ffiSetCustomFps(this.uuid, Math.round(fps));
  }

  // Codec preference is stored as a peer option; the peer re-negotiates the
  // encoder from the refreshed supported-decodings message.
  private pickCodecPreference(value: string): void {
    this.codecPreference = value;
    ffiSessionSetOption(this.uuid, 'codec-preference', value);
    ffiUpdateSupportedDecodings(this.uuid);
  }

  private toggleViewOnly(): void {
    ffiToggleOption(this.uuid, 'view-only');
    this.viewOnly = ffiGetToggleOption(this.uuid, 'view-only') === 1;
  }

  private qualityItems(): RadioItem[] {
    const list: RadioItem[] = [];
    const add = (value: string, label: ResourceStr): void => {
      const it = new RadioItem();
      it.value = value;
      it.label = label;
      it.enabled = true;
      list.push(it);
    };
    add('best', $r('app.string.quality_best'));
    add('balanced', $r('app.string.quality_balanced'));
    add('low', $r('app.string.quality_low'));
    add('custom', $r('app.string.quality_custom'));
    return list;
  }

  private codecItems(): RadioItem[] {
    const list: RadioItem[] = [];
    const add = (value: string, label: ResourceStr, enabled: boolean): void => {
      const it = new RadioItem();
      it.value = value;
      it.label = label;
      it.enabled = enabled;
      list.push(it);
    };
    add('auto', $r('app.string.codec_auto'), true);
    add('vp8', 'VP8', this.altCodecs.vp8);
    add('vp9', 'VP9', true);
    add('h264', 'H264', this.altCodecs.h264);
    add('h265', 'H265', this.altCodecs.h265);
    return list;
  }

  // A remote image clipboard payload: decode the base64 PNG and write it to
  // the system pasteboard as a pixel map so any app can paste it.
  private onClipboardImage(pngBase64: string): void {
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
          pasteboard.getSystemPasteboard().setData(data).then(() => {
            promptAction.showToast({ message: $r('app.string.clipboard_image_received'),
                                     duration: 2000 });
          }).catch((e: BusinessError) => {
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

  // A remote HTML clipboard payload: keep the HTML and its plain-text fallback
  // on the system pasteboard so rich content survives a round-trip. The peer
  // usually sends the text and HTML variants in one MultiClipboards message;
  // the plain variant already went through the clipboard callback, so re-set
  // the record with both records together.
  private onClipboardHtml(html: string): void {
    if (html.length === 0) {
      return;
    }
    try {
      const pb = pasteboard.getSystemPasteboard();
      pb.getData((err: BusinessError, srcData: pasteboard.PasteData) => {
        if (err) {
          hilog.warn(LOG_DOMAIN, TAG, 'pasteboard read failed: %{public}s', `${err}`);
          return;
        }
        try {
          const data = pasteboard.createData(pasteboard.MIMETYPE_TEXT_HTML, html);
          const record = srcData.getRecord(0);
          if (record !== undefined && record !== null &&
              record.mimeType === pasteboard.MIMETYPE_TEXT_PLAIN) {
            data.addRecord(record);
          }
          pb.setData(data).catch((e: BusinessError) => {
            hilog.warn(LOG_DOMAIN, TAG, 'paste html failed: %{public}s', `${e}`);
          });
        } catch (e) {
          hilog.warn(LOG_DOMAIN, TAG, 'pasteboard create html failed: %{public}s', `${e}`);
        }
      });
    } catch (e) {
      hilog.warn(LOG_DOMAIN, TAG, 'clipboard html failed: %{public}s', `${e}`);
    }
  }

  // ------------------------------------------------------------------
  // Quick actions (three-finger tap panel)
  // ------------------------------------------------------------------

  // All entries ride the existing inputKey contract: press=true plus the
  // modifier arguments, so no new FFI surface is needed. CTRL_ALT_DEL /
  // LOCK_SCREEN are peer-side special ControlKeys (Windows: SAS injection /
  // lock workstation).
  private quickActions(): QuickAction[] {
    const list: QuickAction[] = [];
    const add = (label: ResourceStr, onTap: () => void): void => {
      const a = new QuickAction();
      a.label = label;
      a.onTap = onTap;
      list.push(a);
    };
    add($r('app.string.qa_task_manager'), (): void => {
      // Ctrl+Shift+Esc opens the task manager directly on Windows.
      this.ffiInputKey('VK_ESCAPE', false, true, false, true, true, false);
    });
    add($r('app.string.qa_ctrl_alt_del'), (): void => {
      this.ffiInputKey('CTRL_ALT_DEL', false, true, false, false, false, false);
    });
    add($r('app.string.qa_alt_f4'), (): void => {
      this.ffiInputKey('VK_F4', false, true, true, false, false, false);
    });
    add($r('app.string.qa_win'), (): void => {
      this.ffiInputKey('Meta', false, true, false, false, false, false);
    });
    add($r('app.string.qa_win_r'), (): void => {
      this.ffiInputKey('r', false, true, false, false, false, true);
    });
    add($r('app.string.qa_win_d'), (): void => {
      this.ffiInputKey('d', false, true, false, false, false, true);
    });
    add($r('app.string.qa_win_l'), (): void => {
      this.ffiInputKey('l', false, true, false, false, false, true);
    });
    add($r('app.string.qa_lock_screen'), (): void => {
      this.ffiInputKey('LOCK_SCREEN', false, true, false, false, false, false);
    });
    return list;
  }

  private ffiInputKey(name: string, down: boolean, press: boolean,
                      alt: boolean, ctrl: boolean, shift: boolean, command: boolean): void {
    ffiInputKey(this.uuid, name, down, press, alt, ctrl, shift, command);
  }

  // ------------------------------------------------------------------
  // Layout
  // ------------------------------------------------------------------

  private fitVideo(): void {
    if (this.containerW <= 0 || this.containerH <= 0 || this.remoteW <= 0 || this.remoteH <= 0) {
      return;
    }
    const scale = Math.min(this.containerW / this.remoteW, this.containerH / this.remoteH);
    this.videoW = Math.max(1, Math.floor(this.remoteW * scale));
    this.videoH = Math.max(1, Math.floor(this.remoteH * scale));
    this.clampPan();
  }

  // Keep the zoomed viewport inside the container: at scale 1 the video fits,
  // so the offsets collapse to 0; above that the video may overflow by at most
  // half the overflow on each side (it is scaled about its center).
  private clampPan(): void {
    if (this.viewScale <= 1 || this.containerW <= 0 || this.containerH <= 0) {
      this.viewX = 0;
      this.viewY = 0;
      return;
    }
    const maxX = Math.max(0, (this.videoW * this.viewScale - this.containerW) / 2);
    const maxY = Math.max(0, (this.videoH * this.viewScale - this.containerH) / 2);
    if (this.viewX < -maxX) {
      this.viewX = -maxX;
    }
    if (this.viewX > maxX) {
      this.viewX = maxX;
    }
    if (this.viewY < -maxY) {
      this.viewY = -maxY;
    }
    if (this.viewY > maxY) {
      this.viewY = maxY;
    }
  }

  // Signal-bars / shield color in the collapsed status icons: green when the
  // link is good, orange otherwise. Shield green additionally requires an
  // encrypted connection.
  private delayGood(): boolean {
    if (this.connState !== 'ready') {
      return false;
    }
    const ms = parseInt(this.delay, 10);
    return !Number.isNaN(ms) && ms < 100;
  }

  build() {
    Stack() {
      // ---- video area (fullscreen letterbox) ----
      Column() {
        Stack() {
          // Video + remote cursor overlay share one transform (zoom/pan), so
          // the cursor stays glued to the remote pixels it belongs to.
          Stack() {
          XComponent({ id: 'rustdesk_video', type: XComponentType.SURFACE,
                       controller: this.xcController })
            .width('100%')
            .height('100%')
            .onLoad(() => {
              try {
                const sid = this.xcController.getXComponentSurfaceId();
                ffiInitVideo(sid);
              } catch (e) {
                hilog.error(LOG_DOMAIN, TAG, 'initVideo failed: %{public}s', `${e}`);
              }
            })
            .onTouch((e: TouchEvent) => {
              this.onTouchEvent(e);
            })
            // Gestures bound to the video surface only. Exclusive group keeps
            // long-press / pan / tap mutually exclusive; the two-finger
            // pinch-zoom and pan run in parallel with them and with each other.
            .gesture(
              GestureGroup(GestureMode.Exclusive,
                LongPressGesture({ repeat: false, duration: LONG_PRESS_MS })
                  .onAction((e: GestureEvent) => {
                    if (this.pointerMode) {
                      // Trackpad tap-and-hold: hold the right button, lift on
                      // finger release (pan end), like a physical touchpad.
                      this.vptrMouse((MOUSE_BUTTON_RIGHT << 3) | MOUSE_TYPE_DOWN);
                      this.vptrRightHeld = true;
                      return;
                    }
                    this.mouseAt((MOUSE_BUTTON_RIGHT << 3) | MOUSE_TYPE_DOWN,
                                 this.fingerX(e), this.fingerY(e));
                    this.mouseAt((MOUSE_BUTTON_RIGHT << 3) | MOUSE_TYPE_UP,
                                 this.fingerX(e), this.fingerY(e));
                  }),
                // Double tap is detected manually in onClick instead of
                // TapGesture({count: 2}): the tap recognizer never fired for
                // injected touches in E2E testing while onClick did, and
                // count:2 would delay every single tap by the double-tap
                // timeout.
                PanGesture({ fingers: 1, direction: PanDirection.All, distance: 5 })
                  .onActionStart((e: GestureEvent) => {
                    this.lastPtrX = this.fingerX(e);
                    this.lastPtrY = this.fingerY(e);
                    // A multi-finger touch also drives the one-finger
                    // recognizers; park the remote cursor while two-finger
                    // zoom / scroll owns the gesture.
                    if (this.touchTracks.length >= 2 || this.pinching) {
                      return;
                    }
                    if (this.pointerMode) {
                      // Trackpad drag: press only after a tap armed the left
                      // button; a plain drag just moves the cursor.
                      this.vptrDragging = Date.now() - this.lastTapMs < DRAG_AFTER_TAP_MS;
                      if (this.vptrDragging) {
                        this.vptrMouse((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_DOWN);
                      }
                      return;
                    }
                    // Mouse mode (Flutter parity): a plain one-finger drag
                    // only moves the cursor. Left button is pressed solely for
                    // a hold-drag started right after a tap.
                    this.panDragging = Date.now() - this.lastTapMs < DRAG_AFTER_TAP_MS;
                    if (this.panDragging) {
                      this.mouseAt((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_DOWN,
                                   this.fingerX(e), this.fingerY(e));
                    }
                  })
                  .onActionUpdate((e: GestureEvent) => {
                    // Parked while a multi-finger gesture is in flight; see
                    // onActionStart. Keep lastPtr fresh so a pinch that ends
                    // with one finger still down resumes without a jump.
                    if (this.touchTracks.length >= 2 || this.pinching) {
                      this.lastPtrX = this.fingerX(e);
                      this.lastPtrY = this.fingerY(e);
                      return;
                    }
                    if (this.pointerMode) {
                      // Finger travel moves the virtual cursor relatively
                      // (trackpad feel), clamped to the video.
                      this.vptrX += (this.fingerX(e) - this.lastPtrX) * POINTER_SPEED;
                      this.vptrY += (this.fingerY(e) - this.lastPtrY) * POINTER_SPEED;
                      this.lastPtrX = this.fingerX(e);
                      this.lastPtrY = this.fingerY(e);
                      this.vptrClamp();
                      this.vptrMouse(MOUSE_TYPE_MOVE);
                      return;
                    }
                    this.mouseAt(MOUSE_TYPE_MOVE, this.fingerX(e), this.fingerY(e));
                  })
                  .onActionEnd((e: GestureEvent) => {
                    if (this.pointerMode) {
                      if (this.vptrRightHeld) {
                        this.vptrMouse((MOUSE_BUTTON_RIGHT << 3) | MOUSE_TYPE_UP);
                        this.vptrRightHeld = false;
                      }
                      if (this.vptrDragging) {
                        this.vptrMouse((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_UP);
                        this.vptrDragging = false;
                      }
                      return;
                    }
                    if (this.panDragging) {
                      this.mouseAt((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_UP,
                                   this.fingerX(e), this.fingerY(e));
                      this.panDragging = false;
                    }
                  })
                  .onActionCancel(() => {
                    if (this.pointerMode) {
                      if (this.vptrRightHeld) {
                        this.vptrMouse((MOUSE_BUTTON_RIGHT << 3) | MOUSE_TYPE_UP);
                        this.vptrRightHeld = false;
                      }
                      if (this.vptrDragging) {
                        this.vptrMouse((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_UP);
                        this.vptrDragging = false;
                      }
                      return;
                    }
                    if (this.panDragging) {
                      this.mouseAt((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_UP,
                                   this.lastPtrX, this.lastPtrY);
                      this.panDragging = false;
                    }
                  })
              )
            )
            // Left click via onClick instead of TapGesture: the tap recognizer
            // never fired for injected touches in E2E testing, while onClick /
            // pan / long-press all did. ClickEvent x/y are component-local vp,
            // same space as the gesture fingerList coordinates.
            .onClick((e: ClickEvent) => {
              // In pointer mode the finger is a trackpad button: clicks land
              // on the virtual cursor, not on the finger position. Two quick
              // taps = double click; tap-then-drag = hold-drag (arm it).
              if (this.pointerMode) {
                const now = Date.now();
                if (now - this.lastTapMs < DRAG_AFTER_TAP_MS) {
                  this.lastTapMs = 0;
                  this.vptrMouse((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_DOWN);
                  this.vptrMouse((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_UP);
                  this.vptrMouse((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_DOWN);
                  this.vptrMouse((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_UP);
                  return;
                }
                this.lastTapMs = now;
                this.vptrMouse((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_DOWN);
                this.vptrMouse((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_UP);
                return;
              }
              const now = Date.now();
              if (now - this.lastTapMs < DRAG_AFTER_TAP_MS) {
                // Second tap inside the double-tap window: a remote
                // double-click instead of click + hold-drag arming.
                this.lastTapMs = 0;
                this.mouseAt((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_DOWN, e.x, e.y);
                this.mouseAt((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_UP, e.x, e.y);
                return;
              }
              this.lastTapMs = now;
              this.mouseAt((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_DOWN, e.x, e.y);
              this.mouseAt((MOUSE_BUTTON_LEFT << 3) | MOUSE_TYPE_UP, e.x, e.y);
            })
            .parallelGesture(
              GestureGroup(GestureMode.Parallel,
                PinchGesture({ fingers: 2 })
                  .onActionStart((e: GestureEvent) => {
                    this.pinchBase = this.viewScale;
                    this.pinching = true;
                    this.lastFocalX = this.focalX(e);
                    this.lastFocalY = this.focalY(e);
                  })
                  .onActionUpdate((e: GestureEvent) => {
                    let s = this.pinchBase * e.scale;
                    if (s < 1) {
                      s = 1;
                    }
                    if (s > MAX_ZOOM) {
                      s = MAX_ZOOM;
                    }
                    // fingerList localX/Y live in the component's unscaled
                    // space while viewX/Y are container vp, so the focal
                    // delta carries a gain of viewScale. Zoom about the
                    // focal and fold the gain in: a plain focal delta pans
                    // at 1/viewScale, which locks the viewport when zoomed.
                    const fx = this.focalX(e);
                    const fy = this.focalY(e);
                    this.viewX += this.viewScale * (fx - this.videoW / 2) -
                      s * (this.lastFocalX - this.videoW / 2);
                    this.viewY += this.viewScale * (fy - this.videoH / 2) -
                      s * (this.lastFocalY - this.videoH / 2);
                    this.viewScale = s;
                    this.lastFocalX = fx;
                    this.lastFocalY = fy;
                    this.clampPan();
                    this.pokeScrollbars();
                  })
                  .onActionEnd((_e: GestureEvent) => {
                    this.pinching = false;
                  })
                  .onActionCancel(() => {
                    this.pinching = false;
                  }),
                PanGesture({ fingers: 2, direction: PanDirection.All, distance: 5 })
                  .onActionStart((e: GestureEvent) => {
                    this.lastWheelOffsetX = e.offsetX;
                    this.lastWheelOffsetY = e.offsetY;
                  })
                  .onActionUpdate((e: GestureEvent) => {
                    if (this.viewScale > 1) {
                      // Zoomed: while the pinch recognizer holds the touch it
                      // zooms and pans via its focal point; only track the
                      // offsets so a pan that starts after the pinch lifts
                      // resumes smoothly. When the pinch has ended (e.g. one
                      // finger lifted) this pan pans the viewport.
                      if (this.pinching) {
                        this.lastWheelOffsetX = e.offsetX;
                        this.lastWheelOffsetY = e.offsetY;
                      } else {
                        const dx = e.offsetX - this.lastWheelOffsetX;
                        const dy = e.offsetY - this.lastWheelOffsetY;
                        this.lastWheelOffsetX = e.offsetX;
                        this.lastWheelOffsetY = e.offsetY;
                        this.viewX += dx;
                        this.viewY += dy;
                        this.clampPan();
                        this.pokeScrollbars();
                      }
                      return;
                    }
                    // Fit view: two-finger drag scrolls the remote content.
                    // Do not skip while pinching - zoom is clamped to 1 here,
                    // so the pinch cannot act and the wheel is the right
                    // response.
                    const dy = e.offsetY - this.lastWheelOffsetY;
                    this.lastWheelOffsetX = e.offsetX;
                    this.lastWheelOffsetY = e.offsetY;
                    if (dy !== 0) {
                      // Natural scrolling: the content follows the fingers.
                      this.mouse((MOUSE_BUTTON_WHEEL << 3) | MOUSE_TYPE_WHEEL, 0,
                                 Math.round(dy * WHEEL_FACTOR));
                    }
                  })
              )
            )
            // Remote cursor: drawn in video-local vp (remote px * fit scale);
            // the wrapper's zoom transform scales it with the video. The
            // hotspot (hotx/hoty) is the pixel inside the shape that sits on
            // the reported position.
            if (this.remoteCursorDrawn() && this.rcurPm !== undefined) {
              Image(this.rcurPm)
                .width(this.rcurW * this.cursorScale())
                .height(this.rcurH * this.cursorScale())
                .position({
                  x: (this.rcurX - this.rcurHotX) * this.cursorScale(),
                  y: (this.rcurY - this.rcurHotY) * this.cursorScale()
                })
                .hitTestBehavior(HitTestMode.None)
            }
          }
          .width(this.videoW > 0 ? this.videoW : '100%')
          .height(this.videoH > 0 ? this.videoH : '100%')
          .scale({ x: this.viewScale, y: this.viewScale })
          .translate({ x: this.viewX, y: this.viewY })
          // Touch mode draws no local arrow overlay: the peer's real cursor
          // (the remote cursor overlay, or the cursor embedded in the video)
          // already shows where clicks land. Pointer mode still draws its
          // virtual trackpad cursor below.
          if (this.pointerMode) {
            Path()
              .commands('M 0 0 L 0 21 L 5.5 16 L 9 24 L 12.5 22.5 L 9 15 L 16 15 Z')
              .fill('#FFFFFF')
              .stroke('#000000')
              .strokeWidth(1.5)
              .strokeOpacity(0.9)
              .width(24)
              .height(26)
              .position({ x: this.vptrDrawX(), y: this.vptrDrawY() })
              .hitTestBehavior(HitTestMode.None)
          }
          // Scrollbars while zoomed: thin indicator bars on the right/bottom
          // edges of the container showing which part of the video is visible.
          if (this.showScrollbars && this.viewScale > 1 && this.videoW > 0 && this.videoH > 0) {
            Column() {
              ScrollbarIndicator({
                trackLength: this.containerH - 16,
                thumbLength: this.containerH / this.viewScale,
                thumbOffset:
                  -(this.videoTop() * this.viewScale) * (this.containerH - 16) /
                  (this.videoH * this.viewScale - this.containerH),
                vertical: true
              })
              ScrollbarIndicator({
                trackLength: this.containerW - 16,
                thumbLength: this.containerW / this.viewScale,
                thumbOffset:
                  -(this.videoLeft() * this.viewScale) * (this.containerW - 16) /
                  (this.videoW * this.viewScale - this.containerW),
                vertical: false
              })
            }
            .width('100%')
            .height('100%')
            .alignItems(HorizontalAlign.End)
            .justifyContent(FlexAlign.End)
            .hitTestBehavior(HitTestMode.None)
          }
          if (this.connState !== 'ready') {
            Column({ space: 8 }) {
              LoadingProgress().width(36).height(36).color('#FFFFFF')
              Text($r('app.string.connecting')).fontSize(13).fontColor('#CCCCCC')
            }
          }
          if (this.isLandscape) {
            SymbolGlyph($r('sys.symbol.line_3_horizontal'))
              .fontSize(16)
              .fontColor([this.landscapeToolbar ? '#333333' : '#FFFFFF'])
              .width(30)
              .height(30)
              .borderRadius(15)
              .backgroundColor(this.landscapeToolbar ? '#E8E8E8' : 'rgba(0,0,0,0.35)')
              .position({ x: '96%', y: '96%' })
              .markAnchor({ x: '50%', y: '50%' })
              .onClick(() => {
                this.landscapeToolbar = !this.landscapeToolbar;
              })
          }
        }
        .width('100%')
        .layoutWeight(1)
        .backgroundColor('#000000')
        .onAreaChange((_oldArea: Area, newArea: Area) => {
          this.containerW = newArea.width as number;
          this.containerH = newArea.height as number;
          this.fitVideo();
        })
      }
      .width('100%')
      .height('100%')

      // ---- connection status pill (top-left, sunlogin-style): green dot +
      // ---- peer id + connected chevron. Tap to expand the detail card
      // ---- (FPS / speed / delay / permissions).
      if (this.connState === 'ready') {
        Row({ space: 7 }) {
          Circle({ width: 8, height: 8 })
            .fill(this.delayGood() ? '#34C759' : '#FF9800')
          Text(this.peerId)
            .fontSize(14)
            .fontWeight(FontWeight.Medium)
            .fontColor('#FFFFFF')
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.Ellipsis })
            .constraintSize({ maxWidth: 150 })
          Text($r('app.string.connected'))
            .fontSize(12)
            .fontColor('#B8C0CC')
          Text('›')
            .fontSize(15)
            .fontColor('#B8C0CC')
        }
        .padding({ left: 12, right: 10, top: 7, bottom: 7 })
        .borderRadius(20)
        .backgroundColor('rgba(15,18,24,0.62)')
        .position({ x: 10, y: 10 })
        .onClick(() => {
          this.showStatusDetail = !this.showStatusDetail;
        })
      }

      // ---- peer info detail (expanded from the pill, sunlogin style) ----
      if (this.showStatusDetail) {
        Column({ space: 6 }) {
          Row() {
            Text(this.peerName.length > 0 ? this.peerName : this.peerId)
              .fontSize(13)
              .fontWeight(FontWeight.Medium)
              .fontColor('#FFFFFF')
            Blank()
            Text(this.connDetail).fontSize(11).fontColor('#9AA3B0')
          }
          .width('100%')
          Divider().color('rgba(255,255,255,0.14)')
          Row() {
            StatCell({
              caption: $r('app.string.delay'),
              value: this.delay,
              good: this.delayGood()
            })
            StatCell({
              caption: $r('app.string.speed'),
              value: this.speed,
              good: true
            })
            StatCell({
              caption: $r('app.string.fps'),
              value: this.fps,
              good: true
            })
          }
          .width('100%')
          if (this.permissionText.length > 0) {
            Text(this.permissionText).fontSize(10).fontColor('#8A93A0')
              .width('100%')
          }
        }
        .width('86%')
        .padding({ left: 14, right: 14, top: 10, bottom: 10 })
        .borderRadius(14)
        .backgroundColor('rgba(15,18,24,0.78)')
        .position({ x: 10, y: 48 })
        .onClick(() => {
          this.showStatusDetail = false;
        })
      }

      // ---- status badges (top-right: recording / voice call / view-only)
      // ---- plus the virtual-mouse entry. Badges are informational (pass-
      // ---- through hit test); the mouse button below them is clickable.
      Column({ space: 6 }) {
        if (this.recording) {
          Text($r('app.string.record_stop'))
            .fontSize(11)
            .fontColor('#FF8484')
            .padding({ left: 9, right: 9, top: 4, bottom: 4 })
            .borderRadius(12)
            .backgroundColor('rgba(15,18,24,0.6)')
            .hitTestBehavior(HitTestMode.None)
        }
        if (this.voiceCallState !== 'idle') {
          Text(this.voiceCallState === 'active' ? $r('app.string.voice_call_end')
                                                : $r('app.string.voice_call_waiting'))
            .fontSize(11)
            .fontColor('#8FD3A7')
            .padding({ left: 9, right: 9, top: 4, bottom: 4 })
            .borderRadius(12)
            .backgroundColor('rgba(15,18,24,0.6)')
            .hitTestBehavior(HitTestMode.None)
        }
        if (this.viewOnly) {
          Text($r('app.string.view_only'))
            .fontSize(11)
            .fontColor('#FFFFFF')
            .padding({ left: 9, right: 9, top: 4, bottom: 4 })
            .borderRadius(12)
            .backgroundColor('rgba(15,18,24,0.6)')
            .hitTestBehavior(HitTestMode.None)
        }
        // Virtual-mouse entry: tap enters pointer mode / toggles the pad.
        Column() {
          Row()
            .width(2)
            .height(5)
            .borderRadius(1)
            .backgroundColor(this.pointerMode ? '#1A6AF5' : '#FFFFFF')
            .margin({ top: 3 })
          Row()
            .width(14)
            .height(20)
            .borderRadius(7)
            .border({ width: 1.5, color: this.pointerMode ? '#1A6AF5' : '#FFFFFF' })
        }
        .padding({ left: 8, right: 8, top: 5, bottom: 5 })
        .borderRadius(12)
        .backgroundColor('rgba(15,18,24,0.6)')
        .onClick(() => {
          this.toggleVirtualMouse();
        })
      }
      .alignItems(HorizontalAlign.End)
      .position({ x: '100%', y: 10 })
      .markAnchor({ x: '100%', y: 0 })

      // ---- landscape corner chip (unchanged behavior, now re-opens the ball)
      // ----
      if (this.isLandscape) {
        SymbolGlyph($r('sys.symbol.line_3_horizontal'))
          .fontSize(16)
          .fontColor([this.landscapeToolbar ? '#333333' : '#FFFFFF'])
          .width(30)
          .height(30)
          .borderRadius(15)
          .backgroundColor(this.landscapeToolbar ? '#E8E8E8' : 'rgba(0,0,0,0.35)')
          .position({ x: '96%', y: '96%' })
          .markAnchor({ x: '50%', y: '50%' })
          .onClick(() => {
            this.landscapeToolbar = !this.landscapeToolbar;
          })
      }

      // ---- sunlogin-style toolbar: a draggable corner chip ("<>") toggles
      // ---- the bottom icon panel that holds every toolbar function.
      if (!this.isLandscape || this.landscapeToolbar) {
        SymbolGlyph(this.showToolbar ? $r('sys.symbol.xmark') : $r('sys.symbol.ellipsis_bubble'))
          .fontSize(15)
          .fontWeight(FontWeight.Medium)
          .fontColor([this.showToolbar ? '#FFFFFF' : '#1A6AF5'])
          .width(40)
          .height(40)
          .borderRadius(20)
          .backgroundColor(this.showToolbar ? 'rgba(26,106,245,0.9)' : 'rgba(255,255,255,0.92)')
          .shadow({ radius: 8, color: 'rgba(0,0,0,0.25)', offsetY: 2 })
          .position({ x: this.ballX < 0 ? '100%' : this.ballX,
                      y: this.ballY < 0 ? '42%' : this.ballY })
          .markAnchor({ x: '100%', y: '50%' })
          .onClick(() => {
            this.showToolbar = !this.showToolbar;
          })
          .gesture(PanGesture({ fingers: 1, direction: PanDirection.All, distance: 5 })
            .onActionStart((_e: GestureEvent) => {
              this.onBallDragStart();
            })
            .onActionUpdate((e: GestureEvent) => {
              this.onBallDragUpdate(this.ballDragFromX + e.offsetX,
                                    this.ballDragFromY + e.offsetY);
            }))
      }

      // ---- bottom icon panel (the toolbar itself) ----
      if (this.showToolbar) {
        Column() {
          Column({ space: 14 }) {
            Row({ space: 10 }) {
              ToolbarItem({
                icon: $r('sys.symbol.keyboard'),
                label: $r('app.string.keyboard'),
                on: this.showKeyboard,
                onTap: (): void => {
                  this.showToolbar = false;
                  this.showKeyboard = !this.showKeyboard;
                }
              })
              ToolbarItem({
                icon: this.pointerMode ? $r('sys.symbol.hand_tap') : $r('sys.symbol.mouse'),
                label: $r('app.string.quick_mode'),
                on: this.pointerMode,
                onTap: (): void => {
                  this.quickToggleMode();
                }
              })
              ToolbarItem({
                icon: $r('sys.symbol.display'),
                label: $r('app.string.quick_switch_display'),
                onTap: (): void => {
                  this.showToolbar = false;
                  this.quickSwitchDisplay();
                }
              })
              ToolbarItem({
                icon: $r('sys.symbol.slider_horizontal_2_1'),
                label: $r('app.string.display_settings'),
                onTap: (): void => {
                  this.showToolbar = false;
                  this.openDisplaySettings();
                }
              })
            }
            .width('100%')
            Row({ space: 10 }) {
              ToolbarItem({
                icon: $r('sys.symbol.text_clipboard'),
                label: $r('app.string.sync_clipboard'),
                onTap: (): void => {
                  this.showToolbar = false;
                  this.syncClipboard();
                }
              })
              ToolbarItem({
                icon: $r('sys.symbol.phone_down_fill'),
                label: $r('app.string.voice_call_start'),
                on: this.voiceCallState !== 'idle',
                onTap: (): void => {
                  this.showToolbar = false;
                  this.toggleVoiceCall();
                }
              })
              ToolbarItem({
                icon: $r('sys.symbol.record_circle'),
                label: this.recording ? $r('app.string.record_stop')
                                       : $r('app.string.record_start'),
                on: this.recording,
                onTap: (): void => {
                  this.showToolbar = false;
                  this.toggleRecording();
                }
              })
              ToolbarItem({
                icon: $r('sys.symbol.square_grid_2x2'),
                label: $r('app.string.quick_actions'),
                onTap: (): void => {
                  this.showToolbar = false;
                  this.showQuickActions = true;
                }
              })
            }
            .width('100%')
            Divider().color('rgba(255,255,255,0.12)')
            Row({ space: 10 }) {
              ToolbarItem({
                icon: $r('sys.symbol.arrow_counterclockwise'),
                label: $r('app.string.refresh'),
                onTap: (): void => {
                  this.showToolbar = false;
                  ffiRefreshVideo(this.uuid);
                }
              })
              ToolbarItem({
                icon: $r('sys.symbol.lock'),
                label: $r('app.string.view_only'),
                on: this.viewOnly,
                onTap: (): void => {
                  this.toggleViewOnly();
                }
              })
              Blank()
              ToolbarItem({
                icon: $r('sys.symbol.power'),
                label: $r('app.string.disconnect'),
                danger: true,
                onTap: (): void => {
                  this.showToolbar = false;
                  this.disconnect();
                }
              })
            }
            .width('100%')
          }
          .padding({ left: 16, right: 16, top: 16, bottom: 14 })
        }
        .width('100%')
        .borderRadius({ topLeft: 20, topRight: 20 })
        .backgroundColor('rgba(28,32,40,0.96)')
        .position({ x: 0, y: '100%' })
        .markAnchor({ y: '100%' })
        .onClick(() => {
        })
      }
      // Tap the dim area above the panel to close it.
      if (this.showToolbar) {
        Column()
          .width('100%')
          .layoutWeight(1)
          .onClick(() => {
            this.showToolbar = false;
          })
      }

      // ---- remote text input bar (bottom, dark to match the toolbar) ----
      if (this.showKeyboard) {
        Row({ space: 8 }) {
          TextInput({ placeholder: $r('app.string.keyboard_hint'), text: this.keyboardInput })
            .width('72%')
            .height(38)
            .fontSize(14)
            .fontColor('#FFFFFF')
            .placeholderColor('#8A93A0')
            .backgroundColor('rgba(255,255,255,0.12)')
            .onChange((v: string) => {
              this.onKeyboardChange(v);
            })
            .onSubmit(() => {
              this.pressKey('VK_RETURN');
              this.keyboardInput = '';
              this.sentKeyboardText = '';
            })
          Text($r('app.string.key_enter'))
            .fontSize(13)
            .fontColor('#FFFFFF')
            .padding({ left: 12, right: 12, top: 8, bottom: 8 })
            .borderRadius(9)
            .backgroundColor('#1A6AF5')
            .onClick(() => {
              this.pressKey('VK_RETURN');
            })
          SymbolGlyph($r('sys.symbol.chevron_down'))
            .fontSize(15)
            .fontColor(['#FFFFFF'])
            .width(36)
            .height(38)
            .borderRadius(9)
            .backgroundColor('rgba(255,255,255,0.12)')
            .onClick(() => {
              this.showKeyboard = false;
            })
        }
        .width('100%')
        .padding({ left: 12, right: 12, top: 10, bottom: 10 })
        .backgroundColor('rgba(28,32,40,0.96)')
        .position({ x: 0, y: '100%' })
        .markAnchor({ y: '100%' })
      }

      // ---- password dialog ----
      if (this.showPasswordDialog) {
        Column() {
          Column() {
            Text($r('app.string.password_required'))
              .fontSize(18).fontWeight(FontWeight.Medium).margin({ bottom: 8 })
            if (this.msgboxText.length > 0) {
              Text(this.msgboxText).fontSize(13).fontColor('#F44336').margin({ bottom: 8 })
            }
            TextInput({ placeholder: $r('app.string.password_hint'), text: this.passwordInput })
              .type(InputType.Password)
              .width('100%')
              .height(42)
              .onChange((v: string) => {
                this.passwordInput = v;
              })
            Row({ space: 8 }) {
              Toggle({ type: ToggleType.Checkbox, isOn: this.rememberPassword })
                .onChange((v: boolean) => {
                  this.rememberPassword = v;
                })
              Text($r('app.string.remember_password')).fontSize(13)
              Blank()
            }
            .width('100%')
            .margin({ top: 10 })
            Row({ space: 12 }) {
              Button($r('app.string.cancel'), { buttonStyle: ButtonStyleMode.NORMAL })
                .layoutWeight(1)
                .onClick(() => {
                  this.showPasswordDialog = false;
                  this.disconnect();
                })
              Button($r('app.string.ok'))
                .layoutWeight(1)
                .onClick(() => {
                  this.showPasswordDialog = false;
                  ffiSessionLogin(this.uuid, this.passwordInput, this.rememberPassword);
                  this.passwordInput = '';
                })
            }
            .width('100%')
            .margin({ top: 14 })
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

      // ---- 2FA dialog ----
      if (this.showTwofaDialog) {
        Column() {
          Column() {
            Text($r('app.string.twofa_required'))
              .fontSize(18).fontWeight(FontWeight.Medium).margin({ bottom: 8 })
            if (this.msgboxText.length > 0) {
              Text(this.msgboxText).fontSize(13).fontColor('#555555').margin({ bottom: 8 })
            }
            TextInput({ placeholder: $r('app.string.twofa_hint'), text: this.twofaInput })
              .type(InputType.Number)
              .width('100%')
              .height(42)
              .onChange((v: string) => {
                this.twofaInput = v;
              })
            Row({ space: 8 }) {
              Toggle({ type: ToggleType.Checkbox, isOn: this.trustDevice })
                .onChange((v: boolean) => {
                  this.trustDevice = v;
                })
              Text($r('app.string.trust_device')).fontSize(13)
              Blank()
            }
            .width('100%')
            .margin({ top: 10 })
            Row({ space: 12 }) {
              Button($r('app.string.cancel'), { buttonStyle: ButtonStyleMode.NORMAL })
                .layoutWeight(1)
                .onClick(() => {
                  this.showTwofaDialog = false;
                  this.disconnect();
                })
              Button($r('app.string.ok'))
                .layoutWeight(1)
                .onClick(() => {
                  this.showTwofaDialog = false;
                  ffiSessionSend2fa(this.uuid, this.twofaInput, this.trustDevice);
                  this.twofaInput = '';
                })
            }
            .width('100%')
            .margin({ top: 14 })
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

      // ---- error / info dialog (retryable) ----
      if (this.showErrorDialog) {
        Column() {
          Column() {
            Text($r('app.string.error')).fontSize(18).fontWeight(FontWeight.Medium)
              .margin({ bottom: 8 })
            Scroll() {
              Text(this.msgboxText).fontSize(14).fontColor('#555555')
            }
            .constraintSize({ maxHeight: 160 })
            .align(Alignment.Start)
            Row({ space: 12 }) {
              Button($r('app.string.back_home'), { buttonStyle: ButtonStyleMode.NORMAL })
                .layoutWeight(1)
                .onClick(() => {
                  this.showErrorDialog = false;
                  this.disconnect();
                })
              Button($r('app.string.retry'))
                .layoutWeight(1)
                .onClick(() => {
                  this.showErrorDialog = false;
                  ffiSessionReconnect(this.uuid, false);
                })
            }
            .width('100%')
            .margin({ top: 14 })
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

      // ---- incoming voice call dialog (offered by the controlled side) ----
      if (this.showVoiceCallIncomingDialog) {
        Column() {
          Column() {
            Text($r('app.string.share_voice_call_incoming'))
              .fontSize(18).fontWeight(FontWeight.Medium).margin({ bottom: 8 })
            Text(this.peerName.length > 0 ? this.peerName : this.peerId)
              .fontSize(14).fontColor('#555555')
            Row({ space: 12 }) {
              Button($r('app.string.share_reject'), { buttonStyle: ButtonStyleMode.NORMAL })
                .layoutWeight(1)
                .onClick(() => {
                  this.answerVoiceCall(false);
                })
              Button($r('app.string.share_accept'))
                .layoutWeight(1)
                .onClick(() => {
                  this.answerVoiceCall(true);
                })
            }
            .width('100%')
            .margin({ top: 14 })
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

      // ---- clipboard sync dialog (paste, then send) ----
      if (this.showClipboardDialog) {
        Column() {
          Column() {
            Text($r('app.string.sync_clipboard'))
              .fontSize(18).fontWeight(FontWeight.Medium).margin({ bottom: 6 })
            Text($r('app.string.clipboard_paste_hint'))
              .fontSize(12).fontColor('#999999').margin({ bottom: 10 })
            TextArea({ text: this.clipboardInput })
              .width('100%')
              .height(96)
              .fontSize(14)
              .defaultFocus(true)
              .onChange((v: string) => {
                this.clipboardInput = v;
              })
            Row({ space: 12 }) {
              Button($r('app.string.cancel'), { buttonStyle: ButtonStyleMode.NORMAL })
                .layoutWeight(1)
                .onClick(() => {
                  this.showClipboardDialog = false;
                  this.clipboardInput = '';
                })
              Button($r('app.string.send'))
                .layoutWeight(1)
                .onClick(() => {
                  this.sendClipboardInput();
                })
            }
            .width('100%')
            .margin({ top: 14 })
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
      // ---- virtual mouse pad (pointer mode): buttons act on the virtual
      // ---- cursor; drag on the pad body steers the cursor like the video
      // ---- surface does. Bottom-center, above the keyboard bar.
      if (this.showVirtualMouse && this.pointerMode) {
        Column({ space: 10 }) {
          Row({ space: 8 }) {
            Text($r('app.string.virtual_mouse'))
              .fontSize(13)
              .fontWeight(FontWeight.Medium)
              .fontColor('#333333')
              .layoutWeight(1)
            SymbolGlyph($r('sys.symbol.xmark'))
              .fontSize(14)
              .fontColor(['#666666'])
              .onClick(() => {
                this.showVirtualMouse = false;
              })
          }
          .width('100%')
          Row({ space: 8 }) {
            this.VmButton($r('app.string.vm_left'), (): void => {
              this.vmLeftClick();
            })
            this.VmButton($r('app.string.vm_double'), (): void => {
              this.vmDoubleClick();
            })
            this.VmButton($r('app.string.vm_right'), (): void => {
              this.vmRightClick();
            })
          }
          .width('100%')
          Row({ space: 8 }) {
            this.VmButton($r('app.string.vm_middle'), (): void => {
              this.vmMiddleClick();
            })
            this.VmButton($r('app.string.vm_scroll_up'), (): void => {
              this.vmWheel(true);
            })
            this.VmButton($r('app.string.vm_scroll_down'), (): void => {
              this.vmWheel(false);
            })
          }
          .width('100%')
        }
        .width('72%')
        .padding(12)
        .borderRadius(14)
        .backgroundColor('rgba(255,255,255,0.96)')
        .position({ x: '50%', y: '100%' })
        .markAnchor({ x: '50%', y: '100%' })
        .margin({ bottom: 76 })
      }

      // ---- quick actions panel (three-finger tap / toolbar) ----
      if (this.showQuickActions) {
        Column() {
          Column({ space: 2 }) {
            Text($r('app.string.quick_actions'))
              .fontSize(15).fontWeight(FontWeight.Medium)
              .margin({ bottom: 6 })
            ForEach(this.quickActions(), (a: QuickAction) => {
              Text(a.label)
                .fontSize(14)
                .fontColor('#333333')
                .padding({ left: 14, right: 14, top: 9, bottom: 9 })
                .borderRadius(8)
                .width('100%')
                .textAlign(TextAlign.Center)
                .backgroundColor('#F2F3F5')
                .onClick(() => {
                  this.showQuickActions = false;
                  a.onTap();
                })
            }, (_a: QuickAction, idx: number) => `qa_${idx.toString()}`)
            Button($r('app.string.cancel'), { buttonStyle: ButtonStyleMode.NORMAL })
              .width('100%')
              .margin({ top: 10 })
              .onClick(() => {
                this.showQuickActions = false;
              })
          }
          .width('64%')
          .padding(12)
          .borderRadius(12)
          .backgroundColor('#FFFFFF')
        }
        .width('100%')
        .height('100%')
        .backgroundColor('rgba(0,0,0,0.45)')
        .justifyContent(FlexAlign.Center)
        .onClick(() => {
          this.showQuickActions = false;
        })
      }
      // ---- display settings drawer (quality / codec preference) ----
      if (this.showDisplaySettings) {
        Column() {
          Column() {
            Row({ space: 8 }) {
              Text($r('app.string.display_settings'))
                .fontSize(15)
                .fontWeight(FontWeight.Medium)
                .fontColor('#333333')
                .layoutWeight(1)
              Text($r('app.string.close'))
                .fontSize(13)
                .fontColor('#666666')
                .onClick(() => {
                  this.showDisplaySettings = false;
                })
            }
            .width('100%')
            .padding({ left: 16, right: 16, top: 14, bottom: 6 })
            Scroll() {
              Column({ space: 4 }) {
              Text($r('app.string.image_quality'))
                .fontSize(13).fontColor('#666666')
              ForEach(this.qualityItems(), (q: RadioItem) => {
                Row({ space: 8 }) {
                  Radio({ value: q.value, group: 'quality' })
                    .radioStyle({ checkedBackgroundColor: '#1A6AF5' })
                    .onChange((on: boolean) => {
                      if (on) {
                        this.pickImageQuality(q.value);
                      }
                    })
                  Text(q.label).fontSize(14).fontColor('#333333')
                  Blank()
                }
                .width('100%')
                .padding({ left: 6, right: 6, top: 2, bottom: 2 })
              }, (q: RadioItem) => `quality_${q.value}`)
              if (this.imageQuality === 'custom') {
                Column({ space: 2 }) {
                  Row() {
                    Text($r('app.string.bitrate')).fontSize(12).fontColor('#666666')
                    Blank()
                    Text(`${Math.round(this.customQuality)}%`)
                      .fontSize(12).fontColor('#333333')
                  }
                  .width('100%')
                  Slider({ value: this.customQuality, min: MIN_QUALITY, max: MAX_QUALITY,
                           step: 5, style: SliderStyle.OutSet })
                    .width('100%')
                    .onChange((v: number) => {
                      this.applyCustomQuality(v);
                    })
                  Row() {
                    Text($r('app.string.fps_limit')).fontSize(12).fontColor('#666666')
                    Blank()
                    Text(`${Math.round(this.customFps)}`)
                      .fontSize(12).fontColor('#333333')
                  }
                  .width('100%')
                  Slider({ value: this.customFps, min: MIN_FPS, max: MAX_FPS,
                           step: 5, style: SliderStyle.OutSet })
                    .width('100%')
                    .onChange((v: number) => {
                      this.applyCustomFps(v);
                    })
                }
                .width('100%')
                .padding({ left: 6, right: 6 })
              }
              Divider().margin({ top: 4, bottom: 4 })
              Text($r('app.string.codec_preference'))
                .fontSize(13).fontColor('#666666')
              ForEach(this.codecItems(), (c: RadioItem) => {
                Row({ space: 8 }) {
                  Radio({ value: c.value, group: 'codec' })
                    .radioStyle({ checkedBackgroundColor: '#1A6AF5' })
                    .enabled(c.enabled)
                    .onChange((on: boolean) => {
                      if (on) {
                        this.pickCodecPreference(c.value);
                      }
                    })
                  Text(c.label)
                    .fontSize(14)
                    .fontColor(c.enabled ? '#333333' : '#BBBBBB')
                  Blank()
                }
                .width('100%')
                .padding({ left: 6, right: 6, top: 2, bottom: 2 })
              }, (c: RadioItem) => `codec_${c.value}`)
              Divider().margin({ top: 4, bottom: 4 })
              Row({ space: 8 }) {
                Text($r('app.string.remote_cursor')).fontSize(14).fontColor('#333333')
                Blank()
                Toggle({ type: ToggleType.Switch, isOn: this.showRemoteCursor })
                  .selectedColor('#1A6AF5')
                  .onChange((on: boolean) => {
                    if (on !== this.showRemoteCursor) {
                      if (on) {
                        this.showRemoteCursor = true;
                      } else {
                        // Mutually exclusive with pointer mode: switching the
                        // overlay off also leaves pointer mode first.
                        if (this.pointerMode) {
                          this.leavePointerMode();
                        }
                        this.showRemoteCursor = false;
                      }
                    }
                  })
              }
              .width('100%')
              .padding({ left: 6, right: 6 })
              Divider().margin({ top: 4, bottom: 4 })
              Row({ space: 8 }) {
                Text($r('app.string.view_only')).fontSize(14).fontColor('#333333')
                Blank()
                Toggle({ type: ToggleType.Switch, isOn: this.viewOnly })
                  .selectedColor('#1A6AF5')
                  .onChange((on: boolean) => {
                    if (on !== this.viewOnly) {
                      this.toggleViewOnly();
                    }
                  })
              }
              .width('100%')
              .padding({ left: 6, right: 6 })
            }
            .width('100%')
            .padding({ left: 12, right: 12, bottom: 18 })
          }
          .constraintSize({ maxHeight: 480 })
          .align(Alignment.Top)
        }
        .width('100%')
        .padding({ top: 4 })
        .borderRadius({ topLeft: 18, topRight: 18 })
        .backgroundColor('#FFFFFF')
        }
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor('#000000')
    .onAreaChange((_oldArea: Area, newArea: Area) => {
      const w = newArea.width as number;
      const h = newArea.height as number;
      // Orientation flip: the dragged ball position was measured against the
      // old screen size, so the ball may sit off-screen afterwards - reset it
      // back to the default spot for the new orientation.
      const landscape = w > h;
      if (landscape !== this.isLandscape) {
        this.ballX = -1;
        this.ballY = -1;
      }
      this.rootW = w;
      this.rootH = h;
      this.seedBallPos();
      if (this.ballX >= 0) {
        this.onBallDragUpdate(this.ballX, this.ballY);
      }
      this.isLandscape = landscape;
    })
  }

  // Toolbar icon item (sunlogin style): symbol glyph over a caption. A
  // custom component, not a @Builder: by-value @Builder params never
  // re-render when the backing @State changes, so the mode toggle's icon
  // would go stale while the panel stays open. @Prop syncs from the parent.

  // Virtual-mouse pad button: compact filled chip.
  @Builder
  VmButton(label: ResourceStr, onTap: () => void) {
    Text(label)
      .fontSize(13)
      .fontColor('#333333')
      .textAlign(TextAlign.Center)
      .layoutWeight(1)
      .maxLines(1)
      .padding({ top: 10, bottom: 10 })
      .borderRadius(9)
      .backgroundColor('#F2F3F5')
      .onClick(() => {
        onTap();
      })
  }
}
