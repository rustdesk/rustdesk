import { router, promptAction, window } from '@kit.ArkUI';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { BusinessError } from '@kit.BasicServicesKit';
import { image } from '@kit.ImageKit';
import { util } from '@kit.ArkTS';
import { ffiSendMouse, ffiInputKey, ffiInputString, ffiInitVideo, ffiRefreshVideo,
         ffiSwitchDisplay, ffiSessionLogin, ffiSessionSend2fa, ffiSessionReconnect,
         ffiSessionClose, ffiSendClipboard } from '../ohos_ffi';
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

// Toolbar toggle chip: a custom component rather than a @Builder because a
// by-value @Builder param never re-renders when the state variable changes,
// so the selected state would never light up. @Prop syncs from the parent.
@Component
struct ToggleChip {
  @Prop label: ResourceStr = '';
  @Prop on: boolean = false;
  onTap: () => void = (): void => {};

  build() {
    Text(this.label)
      .fontSize(12)
      .padding({ left: 12, right: 12, top: 6, bottom: 6 })
      .borderRadius(6)
      .backgroundColor(this.on ? '#3A7BD5' : '#EEEEEE')
      .fontColor(this.on ? '#FFFFFF' : '#333333')
      .onClick(() => {
        this.onTap();
      })
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
  // Quick-actions panel (three-finger tap / toolbar button).
  @State showQuickActions: boolean = false;
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
        break;
      case 'permission': {
        const parts: string[] = [];
        for (const k of ev.keys) {
          parts.push(`${k}: ${ev.get(k) === 'true' ? 'Y' : 'N'}`);
        }
        this.permissionText = parts.join('  ');
        break;
      }
      case 'update_quality_status':
        this.speed = ev.get('speed');
        this.fps = ev.get('fps');
        this.delay = ev.get('delay');
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
    this.mouse(mask, this.mapX(this.vptrX), this.mapY(this.vptrY));
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
  }

  // Send a mouse event at a finger position (component-local vp). The
  // finger itself steers the pointer; no local arrow overlay is drawn.
  private mouseAt(mask: number, fingerX: number, fingerY: number): void {
    this.mouse(mask, this.mapX(fingerX), this.mapY(fingerY));
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

  private stateColor(): string {
    if (this.connState === 'ready') {
      return '#4CAF50';
    }
    if (this.connState === 'closed' || this.connState === 'failed') {
      return '#F44336';
    }
    return '#FF9800';
  }

  build() {
    Stack() {
      Column() {
        // ---- status bar ----
        Column() {
          Row({ space: 8 }) {
            Text('●').fontSize(12).fontColor(this.stateColor())
            Text(this.peerName.length > 0 ? this.peerName : this.peerId)
              .fontSize(14)
              .fontWeight(FontWeight.Medium)
              .layoutWeight(1)
            Text(this.connState === 'ready' ? $r('app.string.connected') : $r('app.string.connecting'))
              .fontSize(12)
              .fontColor('#666666')
            Text(this.showStatusDetail ? '▾' : '▸').fontSize(12)
          }
          .width('100%')
          .padding({ left: 12, right: 12, top: 8, bottom: 8 })
          .onClick(() => {
            this.showStatusDetail = !this.showStatusDetail;
          })
          if (this.showStatusDetail) {
            Column({ space: 2 }) {
              Row({ space: 6 }) {
                Text($r('app.string.conn_detail')).fontSize(12).fontColor('#666666')
                Text(this.connDetail).fontSize(12).fontColor('#666666')
              }
              Row({ space: 10 }) {
                Text($r('app.string.speed')).fontSize(12).fontColor('#666666')
                Text(this.speed.length > 0 ? this.speed : '-').fontSize(12).fontColor('#666666')
                Text($r('app.string.fps')).fontSize(12).fontColor('#666666')
                Text(this.fps.length > 0 ? this.fps : '-').fontSize(12).fontColor('#666666')
                Text($r('app.string.delay')).fontSize(12).fontColor('#666666')
                Text(this.delay.length > 0 ? this.delay : '-').fontSize(12).fontColor('#666666')
              }
              if (this.permissionText.length > 0) {
                Text(this.permissionText).fontSize(11).fontColor('#999999')
              }
            }
            .width('100%')
            .alignItems(HorizontalAlign.Start)
            .padding({ left: 30, right: 12, bottom: 8 })
          }
        }
        .width('100%')
        .backgroundColor('#F2F3F5')

        // ---- video area (letterbox) ----
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
            Text('☰')
              .fontSize(16)
              .fontColor(this.landscapeToolbar ? '#333333' : '#FFFFFF')
              .textAlign(TextAlign.Center)
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

        // ---- toolbar (auto-hidden in landscape; the corner chip brings it
        // back) ----
        if (!this.isLandscape || this.landscapeToolbar) {
          Column({ space: 6 }) {
          if (this.showKeyboard) {
            TextInput({ placeholder: $r('app.string.keyboard_hint'), text: this.keyboardInput })
              .width('100%')
              .height(40)
              .fontSize(15)
              .onChange((v: string) => {
                this.onKeyboardChange(v);
              })
              .onSubmit(() => {
                this.pressKey('VK_RETURN');
                this.keyboardInput = '';
                this.sentKeyboardText = '';
              })
          }
          Row({ space: 8 }) {
            ToggleChip({
              label: 'Ctrl',
              on: this.ctrlOn,
              onTap: (): void => {
                this.ctrlOn = !this.ctrlOn;
              }
            })
            ToggleChip({
              label: 'Alt',
              on: this.altOn,
              onTap: (): void => {
                this.altOn = !this.altOn;
              }
            })
            ToggleChip({
              label: 'Shift',
              on: this.shiftOn,
              onTap: (): void => {
                this.shiftOn = !this.shiftOn;
              }
            })
            Blank()
            this.ToolButton($r('app.string.keyboard'), (): void => {
              this.showKeyboard = !this.showKeyboard;
            })
          }
          .width('100%')
          Row({ space: 8 }) {
            ToggleChip({
              label: $r('app.string.remote_cursor'),
              on: this.showRemoteCursor,
              onTap: (): void => {
                // Mutually exclusive with pointer mode: tapping this chip
                // while it is on leaves pointer mode (which re-enables it).
                if (this.showRemoteCursor) {
                  this.leavePointerMode();
                  this.showRemoteCursor = false;
                  return;
                }
                this.showRemoteCursor = true;
              }
            })
            ToggleChip({
              label: $r('app.string.pointer_mode'),
              on: this.pointerMode,
              onTap: (): void => {
                if (this.pointerMode) {
                  this.leavePointerMode();
                } else {
                  this.enterPointerMode();
                }
              }
            })
            this.ToolButton($r('app.string.quick_actions'), (): void => {
              this.showQuickActions = true;
            })
            this.ToolButton($r('app.string.sync_clipboard'), (): void => {
              this.syncClipboard();
            })
            Blank()
          }
          .width('100%')
          Row({ space: 8 }) {
            this.ToolButton('Esc', (): void => {
              this.pressKey('VK_ESCAPE');
            })
            this.ToolButton('Tab', (): void => {
              this.pressKey('VK_TAB');
            })
            this.ToolButton($r('app.string.key_enter'), (): void => {
              this.pressKey('VK_RETURN');
            })
            this.ToolButton($r('app.string.key_backspace'), (): void => {
              this.pressKey('VK_BACK');
            })
            Blank()
            this.ToolButton($r('app.string.refresh'), (): void => {
              ffiRefreshVideo(this.uuid);
            })
            this.ToolButton($r('app.string.disconnect'), (): void => {
              this.disconnect();
            })
          }
          .width('100%')
          if (this.displays.length > 1) {
            Row({ space: 8 }) {
              Text($r('app.string.display')).fontSize(12).fontColor('#666666')
              ForEach(this.displays, (_d: DisplayItem, idx: number) => {
                Text(`${idx + 1}`)
                  .fontSize(12)
                  .padding({ left: 10, right: 10, top: 4, bottom: 4 })
                  .borderRadius(6)
                  .backgroundColor(idx === this.currentDisplay ? '#3A7BD5' : '#F2F3F5')
                  .fontColor(idx === this.currentDisplay ? '#FFFFFF' : '#333333')
                  .onClick(() => {
                    ffiSwitchDisplay(this.uuid, idx);
                  })
              }, (_d: DisplayItem, idx: number) => `display_${idx}`)
            }
            .width('100%')
          }
        }
          .width('100%')
          .padding({ left: 12, right: 12, top: 6, bottom: 10 })
          .backgroundColor('#FAFAFA')
        }
      }
      .width('100%')
      .height('100%')

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
    }
    .width('100%')
    .height('100%')
    .backgroundColor('#000000')
    .onAreaChange((_oldArea: Area, newArea: Area) => {
      this.isLandscape = (newArea.width as number) > (newArea.height as number);
    })
  }

  @Builder
  ToolButton(label: ResourceStr, onTap: () => void) {
    Text(label)
      .fontSize(12)
      .padding({ left: 10, right: 10, top: 6, bottom: 6 })
      .borderRadius(6)
      .backgroundColor('#EEEEEE')
      .fontColor('#333333')
      .onClick(() => {
        onTap();
      })
  }
}
