import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { flushSync } from 'react-dom';
import { MousePointer2 } from 'lucide-react';
import { RemoteAudio } from './lib/remoteAudio';
import { DiagnosticOverlay } from './components/DiagnosticOverlay';
import type { MobilePanel } from './components/MobileSessionPanel';
import { MobileDock } from './components/MobileDock';
import {
  RustDeskShell,
  type ConnectRequest,
  type ConnectionMode,
  type ConnectionPhase,
} from './components/RustDeskShell';
import { SessionHandle } from './components/SessionHandle';
import { SessionMenu, type SessionStats } from './components/SessionMenu';
import { SoftKeyboardBar } from './components/SoftKeyboardBar';
import { probeCodecs, RemoteDecoder, webCodecsStatus } from './lib/decoder';
import {
  deviceKeyFor,
  loadSavedDevices,
  persistSavedDevices,
  removeDevice,
  toggleFavoriteDevice,
  upsertRecentDevice,
  type SavedDevice,
} from './lib/devices';
import {
  clearLegacyPasswordStorage,
  removeSavedPassword,
  setSavedPassword,
  type SavedPasswordMap,
} from './lib/passwords';
import {
  applyMouseButtonSwap,
  applyWheelDirection,
  filterCodecSupport,
  loadDisplayPrefs,
  persistDisplayPrefs,
  resolveQualityLevel,
  resolveTargetFps,
  type DisplayPrefs,
} from './lib/displayPrefs';
import {
  computeDiagnosticsSummary,
  createDiagnosticsState,
  PING_INTERVAL_MS,
  PING_TIMEOUT_MS,
  recordPingSent,
  recordPongReceived,
  sweepTimeouts,
  type DiagnosticsState,
  type DiagnosticsSummary,
} from './lib/diagnostics';
import {
  accumulateWheelTicks,
  createTouchGestureState,
  gestureLongPressTimeout,
  gesturePointerDown,
  gesturePointerMove,
  gesturePointerUpOrCancel,
  isPageZoomed,
  keyboardInset,
  LONG_PRESS_MS,
  MAX_WHEEL_TICKS_PER_EVENT,
  resetTouchGestureState,
  type TouchGestureEffect,
  type TouchGestureState,
} from './lib/gestures';
import {
  LOCALE_STORAGE_KEY,
  resolveEffectiveLocale,
  resolveLocalePreference,
  t,
  type I18nKey,
  type Locale,
  type LocalePreference,
} from './lib/i18n';
import {
  isBrowserNavOrAuxButton,
  shouldIsolateKeyboardEvent,
  shouldPreventStageTouch,
  shouldPreventWindowWheel,
} from './lib/inputIsolation';
import {
  isImeEvent,
  isModifier,
  isPrintableAscii,
  mapPhysicalKeyDown,
  mapPhysicalKeyUp,
} from './lib/keys';
import {
  DEFAULT_EDGE_PADDING,
  clampPan,
  computeNextScaleState,
  computePinchStep,
  computeViewTransform,
  fitScale,
  mapFromRemote,
  mapToRemote,
  mapToRemoteClamped,
  type ViewMode,
  type ViewTransform,
} from './lib/mapping';
import {
  decodeFrame,
  formatBitrate,
  wsUrl,
  type ClientMessage,
  type PeerMessage,
  type QualityLevel,
  type ServerMessage,
} from './lib/protocol';
import {
  computeFrameRates,
  createFrameStats,
  recordRenderedFrame,
  recordRxFrame,
  type FrameStatsState,
} from './lib/stats';
import {
  loadSavedTheme,
  persistTheme,
  resolveEffectiveTheme,
  type ThemePreference,
} from './lib/theme';

const EMPTY_STATS: SessionStats = {
  codec: '—',
  width: 0,
  height: 0,
  fps: 0,
  rxFps: 0,
  delayMs: 0,
  bitrate: 0,
  decoded: 0,
  dropped: 0,
  streamState: 'waiting',
};

function detectCoarsePointer(): boolean {
  if (typeof window === 'undefined') return false;
  return (
    window.matchMedia?.('(pointer: coarse)').matches ||
    (typeof navigator !== 'undefined' && navigator.maxTouchPoints > 0)
  );
}

const PREFS_STORAGE_KEY = 'rd_web_prefs';

interface SavedConnectionPrefs {
  mode?: ConnectionMode;
  directIpEnabled?: boolean;
  host?: string;
  port?: string;
  peerId?: string;
  idServer?: string;
  relayServer?: string;
  serverKey?: string;
  forceRelay?: boolean;
}

function loadSavedPrefs(): SavedConnectionPrefs {
  try {
    const raw = window.localStorage.getItem(PREFS_STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as SavedConnectionPrefs;
    return parsed && typeof parsed === 'object' ? parsed : {};
  } catch {
    return {};
  }
}

export default function App() {
  const initialWebCodecs = webCodecsStatus();
  const isCoarseDevice = detectCoarsePointer();
  const initialPrefs = useRef<SavedConnectionPrefs | null>(null);
  if (initialPrefs.current === null) {
    initialPrefs.current = loadSavedPrefs();
  }
  const prefs = initialPrefs.current;

  // Only non-secret connection preferences are persisted to browser storage.
  const [directIpEnabled, setDirectIpEnabled] = useState<boolean>(Boolean(prefs.directIpEnabled));
  const [mode, setMode] = useState<ConnectionMode>(
    prefs.directIpEnabled && prefs.mode === 'direct' ? 'direct' : 'id',
  );
  const [host, setHost] = useState(typeof prefs.host === 'string' ? prefs.host : '');
  const [port, setPort] = useState(typeof prefs.port === 'string' ? prefs.port : '21118');
  const [password, setPassword] = useState('');
  const [peerId, setPeerId] = useState(typeof prefs.peerId === 'string' ? prefs.peerId : '');
  const [idServer, setIdServer] = useState(typeof prefs.idServer === 'string' ? prefs.idServer : '');
  const [relayServer, setRelayServer] = useState(
    typeof prefs.relayServer === 'string' ? prefs.relayServer : '',
  );
  const [serverKey, setServerKey] = useState(
    typeof prefs.serverKey === 'string' ? prefs.serverKey : '',
  );
  const [forceRelay, setForceRelay] = useState(Boolean(prefs.forceRelay));
  const [gatewayToken, setGatewayToken] = useState('');
  const [authRequired, setAuthRequired] = useState(false);
  const [devices, setDevices] = useState<SavedDevice[]>(() => loadSavedDevices());

  useEffect(() => {
    try {
      const next: SavedConnectionPrefs = {
        mode: directIpEnabled ? mode : 'id',
        directIpEnabled,
        host,
        port,
        peerId,
        idServer,
        relayServer,
        serverKey,
        forceRelay,
      };
      window.localStorage.setItem(PREFS_STORAGE_KEY, JSON.stringify(next));
    } catch {
      // Ignore storage quota or private-mode errors
    }
  }, [mode, directIpEnabled, host, port, peerId, idServer, relayServer, serverKey, forceRelay]);

  const handleToggleDirectIp = useCallback(() => {
    setDirectIpEnabled((prev) => {
      const next = !prev;
      if (!next) {
        setMode('id');
      }
      return next;
    });
  }, []);

  useEffect(() => {
    persistSavedDevices(devices);
  }, [devices]);

  const handleSelectDevice = useCallback((device: SavedDevice) => {
    if (device.mode === 'direct') {
      setDirectIpEnabled(true);
    }
    setMode(device.mode);
    if (device.mode === 'id') {
      if (device.peerId) setPeerId(device.peerId);
      if (device.idServer) setIdServer(device.idServer);
      if (typeof device.relayServer === 'string') setRelayServer(device.relayServer);
      if (typeof device.serverKey === 'string') setServerKey(device.serverKey);
      if (typeof device.forceRelay === 'boolean') setForceRelay(device.forceRelay);
    } else {
      if (device.host) setHost(device.host);
      if (device.port) setPort(device.port);
    }
  }, []);

  const [savedPasswords, setSavedPasswords] = useState<SavedPasswordMap>({});
  const [rememberPassword, setRememberPassword] = useState(false);

  useEffect(() => {
    clearLegacyPasswordStorage();
  }, []);

  const handleRememberPasswordChange = (enabled: boolean) => {
    setRememberPassword(enabled);
    if (!enabled) setSavedPasswords({});
  };

  const handleToggleFavorite = useCallback((key: string) => {
    setDevices((prev) => toggleFavoriteDevice(prev, key));
  }, []);

  const handleRemoveDevice = useCallback((key: string) => {
    setDevices((prev) => removeDevice(prev, key));
    setSavedPasswords((prev) => removeSavedPassword(prev, key));
  }, []);

  const handleClearSavedPassword = useCallback((key: string) => {
    setSavedPasswords((prev) => removeSavedPassword(prev, key));
  }, []);

  // Bilingual UI locale preference ('default' follows browser language, or explicit 'zh'/'en')
  const [localePreference, setLocalePreference] = useState<LocalePreference>(() => {
    try {
      return resolveLocalePreference(window.localStorage.getItem(LOCALE_STORAGE_KEY));
    } catch {
      return 'default';
    }
  });

  const locale: Locale = resolveEffectiveLocale(
    localePreference,
    typeof navigator !== 'undefined' ? navigator.language : null,
  );

  const changeLocale = useCallback((next: LocalePreference) => {
    setLocalePreference(next);
    try {
      window.localStorage.setItem(LOCALE_STORAGE_KEY, next);
    } catch {
      // Ignore storage quota or private-mode errors
    }
  }, []);

  useEffect(() => {
    document.documentElement.lang = locale === 'zh' ? 'zh-CN' : 'en';
  }, [locale]);

  // Theme preference (light / dark / system) synced to <html data-theme="...">
  const [theme, setTheme] = useState<ThemePreference>(() => loadSavedTheme());

  const changeTheme = useCallback((next: ThemePreference) => {
    setTheme(next);
    persistTheme(next);
  }, []);

  useEffect(() => {
    const mq = window.matchMedia?.('(prefers-color-scheme: dark)');
    const apply = () => {
      const effective = resolveEffectiveTheme(theme, Boolean(mq?.matches));
      document.documentElement.dataset.theme = effective;
    };
    apply();
    if (theme === 'system' && mq) {
      mq.addEventListener?.('change', apply);
      return () => mq.removeEventListener?.('change', apply);
    }
  }, [theme]);

  const [phase, setPhase] = useState<ConnectionPhase>('idle');
  const [detailKey, setDetailKey] = useState<I18nKey | null>('detailIdle');
  const [rawDetail, setRawDetail] = useState('');
  const detail = detailKey ? t(locale, detailKey) : rawDetail;
  const setStatusDetail = useCallback((key: I18nKey | null, customText = '') => {
    setDetailKey(key);
    setRawDetail(customText);
  }, []);
  // Display preferences (persisted locally & wired end-to-end to view, quality, codec, and input)
  const [displayPrefs, setDisplayPrefs] = useState<DisplayPrefs>(() => loadDisplayPrefs());
  const displayPrefsRef = useRef(displayPrefs);
  displayPrefsRef.current = displayPrefs;

  const [resolutionOptions, setResolutionOptions] = useState<Extract<ServerMessage, { type: 'resolutions' }> | null>(null);
  const followCursorRef = useRef<(x: number, y: number) => void>(() => {});
  const [peer, setPeer] = useState<PeerMessage | null>(null);
  const [display, setDisplay] = useState(0);
  const [stats, setStats] = useState<SessionStats>(EMPTY_STATS);
  const [decoderError, setDecoderError] = useState(initialWebCodecs.ok ? '' : initialWebCodecs.reason);
  const [rtt, setRtt] = useState(0);
  const [quality, setQuality] = useState<QualityLevel>(() =>
    resolveQualityLevel(displayPrefs.qualityPreset, displayPrefs.customBitrate),
  );
  const [targetFps, setTargetFps] = useState(() =>
    resolveTargetFps(displayPrefs.qualityPreset, displayPrefs.customFps),
  );
  const [remoteClipboard, setRemoteClipboard] = useState('');

  // Viewport & chrome state
  const [viewMode, setViewMode] = useState<ViewMode>(() => displayPrefs.viewStyle);
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ panX: 0, panY: 0 });
  const [hand, setHand] = useState(false);
  const [touchMode, setTouchMode] = useState<boolean>(isCoarseDevice);
  const [rightArmed, setRightArmed] = useState(false);
  const [handleX, setHandleX] = useState(0.5);
  const [toolbarExpanded, setToolbarExpanded] = useState(false);
  const [toolbarPinned, setToolbarPinned] = useState(false);
  const [box, setBox] = useState({ w: 0, h: 0 });
  const [mobilePanel, setMobilePanel] = useState<MobilePanel>('toolbar');
  const [menuOpen, setMenuOpen] = useState(false);
  const [connectDialog, setConnectDialog] = useState(false);
  const [softKeys, setSoftKeys] = useState(false);
  const [virtualCursorVisible, setVirtualCursorVisible] = useState(false);
  const [cursorEmbedded, setCursorEmbedded] = useState(false);
  const [cursorShape, setCursorShape] = useState<{
    pixels: ImageData;
    hotX: number;
    hotY: number;
  } | null>(null);
  const textCursor = useMemo(() => {
    if (!cursorShape || cursorEmbedded || !displayPrefs.autoKeyboard) return false;
    const { pixels, hotX, hotY } = cursorShape;
    // Ignore transparent padding. An I-beam has narrow middle rows and wider caps.
    const widths: number[] = [];
    let left = pixels.width, right = -1;
    for (let y = 0; y < pixels.height; y++) {
      let first = pixels.width, last = -1;
      for (let x = 0; x < pixels.width; x++) {
        if (pixels.data[(y * pixels.width + x) * 4 + 3] < 96) continue;
        first = Math.min(first, x);
        last = x;
      }
      widths.push(Math.max(0, last - first + 1));
      left = Math.min(left, first);
      right = Math.max(right, last);
    }
    const top = widths.findIndex(w => w > 0);
    if (top < 0) return false;
    let bottom = widths.length - 1;
    while (!widths[bottom]) bottom--;
    const height = bottom - top + 1, width = right - left + 1;
    const cap = Math.max(1, Math.floor(height / 4));
    const stem = widths.slice(top + cap, bottom - cap + 1);
    return height >= 8 && height >= width * 1.4 &&
      Math.abs(hotX - (left + right) / 2) <= Math.max(1, width / 4) &&
      Math.abs(hotY - (top + bottom) / 2) <= height / 3 &&
      stem.every(w => w > 0 && w <= width * 0.7) &&
      Math.max(...widths.slice(top, top + cap)) >= width * 0.8 &&
      Math.max(...widths.slice(bottom - cap + 1, bottom + 1)) >= width * 0.8;
  }, [cursorShape, cursorEmbedded, displayPrefs.autoKeyboard]);
  const [pageZoomed, setPageZoomed] = useState(false);
  const [qualityMonitor, setQualityMonitor] = useState(false);
  const [restartState, setRestartState] = useState<'requested' | 'waiting' | null>(null);
  const restartDeadline = useRef(0);
  const reconnectRequest = useRef<ConnectRequest | null>(null);
  const connectGeneration = useRef(0);
  const routeLabel = t(locale, peer?.route === 'relay' ? 'routeRelay' : peer?.route === 'direct' ? 'routeDirect' : 'routeUnknown');
  const [diagnosticsOpen, setDiagnosticsOpen] = useState<boolean>(() => {
    try {
      return new URLSearchParams(window.location.search).get('diag') === '1';
    } catch {
      return false;
    }
  });
  const diagStateRef = useRef<DiagnosticsState>(createDiagnosticsState());
  const diagSeqRef = useRef(0);
  const [diagSummary, setDiagSummary] = useState<DiagnosticsSummary>(() =>
    computeDiagnosticsSummary(diagStateRef.current, performance.now()),
  );

  // Refs
  const stageRef = useRef<HTMLElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const cursorElRef = useRef<HTMLDivElement>(null);
  const cursorCanvasRef = useRef<HTMLCanvasElement>(null);
  const captureRef = useRef<HTMLInputElement>(null);
  const softKeyboardInputRef = useRef<HTMLInputElement>(null);
  const captureComposingRef = useRef(false);
  const lastCompositionEndAtRef = useRef(0);
  const pressedChrByCodeRef = useRef<Map<string, string>>(new Map());
  const wsRef = useRef<WebSocket | null>(null);
  const decoderRef = useRef<RemoteDecoder | null>(null);
  const sizeRef = useRef({ width: 1280, height: 720 });
  const phaseRef = useRef<ConnectionPhase>(phase);
  phaseRef.current = phase;

  const chromeRef = useRef({ menu: false, dialog: true });
  chromeRef.current = { menu: menuOpen, dialog: connectDialog || phase !== 'live' };

  // Frame sampling-rate telemetry refs (avoiding 60Hz React setState per frame)
  const frameStatsRef = useRef<FrameStatsState>(createFrameStats());
  const statAccRef = useRef({
    width: 0,
    height: 0,
    decoded: 0,
    dropped: 0,
    lastDelayMs: 0,
    lastCodec: '—',
  });

  // Mouse & touch gesture refs (unified deterministic TouchGestureState FSM)
  const cursorRef = useRef({ x: 640, y: 360 });
  const touchGestureRef = useRef<TouchGestureState>(createTouchGestureState());
  const touchFallbackPinchRef = useRef<{ midX: number; midY: number; dist: number } | null>(null);
  const nativeTouchInfoRef = useRef<{ count: number; updatedAt: number }>({ count: 0, updatedAt: 0 });
  const zeroTouchTimerRef = useRef<number | null>(null);
  const wheelAccumRef = useRef({ x: 0, y: 0 });
  const lastTouchAtRef = useRef(0);
  const longTimerRef = useRef<number | null>(null);
  const touchModeRef = useRef(touchMode);
  touchModeRef.current = touchMode;
  const handRef = useRef(hand);
  handRef.current = hand;
  const rightArmedRef = useRef(rightArmed);
  rightArmedRef.current = rightArmed;
  const pendingClickRef = useRef<{
    button: 'left' | 'right' | 'middle';
    x: number;
    y: number;
    timer: number;
  } | null>(null);

  // Desktop pointer state
  const desktopDragRef = useRef<{
    pointerId: number;
    mode: 'remote' | 'pan';
    button: 'left' | 'right' | 'middle';
    lastX: number;
    lastY: number;
  } | null>(null);

  // Throttled rAF mouse move queue
  const queuedMoveRef = useRef<{ x: number; y: number } | null>(null);
  const moveRafRef = useRef<number | null>(null);

  // Orientation change tracking
  const prevLandscapeRef = useRef<boolean | null>(null);

  const panClampOptions = useMemo(
    () => ({
      allowEdgePan: true,
      edgePadding: DEFAULT_EDGE_PADDING,
      bottomInset: softKeys ? 46 : 0,
    }),
    [softKeys],
  );

  const remoteW = stats.width > 0 ? stats.width : sizeRef.current.width;
  const remoteH = stats.height > 0 ? stats.height : sizeRef.current.height;
  const view: ViewTransform = computeViewTransform(
    box.w,
    box.h,
    remoteW,
    remoteH,
    viewMode,
    zoom,
    pan,
    panClampOptions,
  );
  const viewRef = useRef(view);
  viewRef.current = view;

  const remoteInput = phase === 'live' && !menuOpen && !connectDialog;
  const remoteInputRef = useRef(remoteInput);
  remoteInputRef.current = remoteInput;
  const zoomPercent = Math.round(view.scale * 100);

  // Send JSON helper
  const send = useCallback((msg: ClientMessage) => {
    const sock = wsRef.current;
    if (sock && sock.readyState === WebSocket.OPEN) {
      sock.send(JSON.stringify(msg));
    }
  }, []);

  useEffect(() => {
    if (phase === 'live' && peer && peer.displays.length > 1) {
      send({ type: 'followWindow', enabled: displayPrefs.followWindow });
    }
  }, [phase, peer, displayPrefs.followWindow, send]);

  const audioRef = useRef<RemoteAudio | null>(null);
  const [audioEnabled, setAudioEnabled] = useState(false);
  const [audioIssue, setAudioIssue] = useState<'audioUnsupported' | 'audioDenied' | 'audioFailed' | null>(null);
  const stopAudio = useCallback(() => {
    const player = audioRef.current;
    audioRef.current = null;
    player?.close();
    setAudioEnabled(false);
  }, []);
  useEffect(() => {
    if (phase !== 'live') {
      stopAudio();
      setAudioIssue(null);
    }
    return () => { audioRef.current?.close(); audioRef.current = null; };
  }, [phase, stopAudio]);

  const toggleAudio = async () => {
    setAudioIssue(null);
    if (audioRef.current) {
      stopAudio();
      send({ type: 'audio', enabled: false });
      return;
    }
    if (typeof AudioDecoder === 'undefined' || typeof AudioContext === 'undefined') {
      setAudioIssue('audioUnsupported');
      return;
    }
    const socket = wsRef.current;
    let player: RemoteAudio;
    const fail = () => {
      if (socket !== wsRef.current || (player && player !== audioRef.current)) return;
      stopAudio();
      setAudioIssue('audioFailed');
      send({ type: 'audio', enabled: false });
    };
    try {
      player = new RemoteAudio(fail);
      audioRef.current = player;
      setAudioEnabled(true);
      await player.resume();
      if (player === audioRef.current && socket === wsRef.current) send({ type: 'audio', enabled: true });
    } catch {
      fail();
    }
  };

  const cancelQueuedMove = useCallback(() => {
    if (moveRafRef.current !== null) {
      cancelAnimationFrame(moveRafRef.current);
      moveRafRef.current = null;
    }
    queuedMoveRef.current = null;
  }, []);

  const flushMove = useCallback(() => {
    if (moveRafRef.current !== null) {
      cancelAnimationFrame(moveRafRef.current);
      moveRafRef.current = null;
    }
    const pos = queuedMoveRef.current;
    queuedMoveRef.current = null;
    if (pos && !chromeRef.current.menu && !chromeRef.current.dialog) {
      send({ type: 'mouse', action: 'move', x: pos.x, y: pos.y });
    }
  }, [send]);

  const queueMove = useCallback(
    (pos: { x: number; y: number }) => {
      queuedMoveRef.current = pos;
      if (moveRafRef.current === null) {
        moveRafRef.current = requestAnimationFrame(() => {
          moveRafRef.current = null;
          const next = queuedMoveRef.current;
          queuedMoveRef.current = null;
          if (next && !chromeRef.current.menu && !chromeRef.current.dialog) {
            send({ type: 'mouse', action: 'move', x: next.x, y: next.y });
          }
        });
      }
    },
    [send],
  );

  const finishPendingClick = useCallback(() => {
    const pending = pendingClickRef.current;
    if (!pending) return;
    window.clearTimeout(pending.timer);
    pendingClickRef.current = null;
    send({
      type: 'mouse',
      action: 'up',
      button: pending.button,
      x: pending.x,
      y: pending.y,
    });
  }, [send]);

  const clearLongPress = useCallback(() => {
    if (longTimerRef.current !== null) {
      window.clearTimeout(longTimerRef.current);
      longTimerRef.current = null;
    }
  }, []);

  const cursorPoint = useCallback(() => {
    const w = Math.max(1, sizeRef.current.width);
    const h = Math.max(1, sizeRef.current.height);
    return {
      x: Math.min(w - 1, Math.max(0, Math.round(cursorRef.current.x))),
      y: Math.min(h - 1, Math.max(0, Math.round(cursorRef.current.y))),
    };
  }, []);

  const placeCursor = useCallback((x: number, y: number) => {
    const w = Math.max(1, sizeRef.current.width);
    const h = Math.max(1, sizeRef.current.height);
    const cx = Math.min(w - 1, Math.max(0, x));
    const cy = Math.min(h - 1, Math.max(0, y));
    cursorRef.current = { x: cx, y: cy };
    if (displayPrefsRef.current.followRemoteCursor) followCursorRef.current(cx, cy);
    if (window.__rd) {
      window.__rd.cursorX = Math.round(cx);
      window.__rd.cursorY = Math.round(cy);
    }
    const el = cursorElRef.current;
    if (el) {
      const screen = mapFromRemote(cx, cy, viewRef.current);
      if (el.style.visibility) el.style.removeProperty('visibility');
      el.style.left = `${screen.x}px`;
      el.style.top = `${screen.y}px`;
    }
  }, []);

  const clickButton = useCallback(
    (button: 'left' | 'right' | 'middle', x: number, y: number) => {
      if (displayPrefsRef.current.viewOnly) return;
      const mapped = applyMouseButtonSwap(button, displayPrefsRef.current.swapMouseButtons);
      finishPendingClick();
      if (touchModeRef.current) placeCursor(x, y);
      send({ type: 'mouse', action: 'move', x, y });
      send({ type: 'mouse', action: 'down', button: mapped, x, y });
      const timer = window.setTimeout(() => {
        pendingClickRef.current = null;
        send({ type: 'mouse', action: 'up', button: mapped, x, y });
      }, 40);
      pendingClickRef.current = { button: mapped, x, y, timer };
    },
    [finishPendingClick, placeCursor, send],
  );

  // Keep virtual cursor DOM position synchronized on view changes
  useEffect(() => {
    const el = cursorElRef.current;
    if (!el) return;
    const screen = mapFromRemote(cursorRef.current.x, cursorRef.current.y, view);
    el.style.left = `${screen.x}px`;
    el.style.top = `${screen.y}px`;
  }, [view.scale, view.panX, view.panY, virtualCursorVisible]);

  useEffect(() => {
    if (cursorShape) {
      cursorCanvasRef.current?.getContext('2d')?.putImageData(cursorShape.pixels, 0, 0);
    }
  }, [cursorShape]);

  // Expose window.__rd and window.__rdView on every render
  useEffect(() => {
    window.__rd = {
      state: phase,
      codec: stats.codec,
      width: stats.width,
      height: stats.height,
      fps: stats.fps,
      delayMs: rtt || stats.delayMs,
      bitrate: stats.bitrate,
      decoded: stats.decoded,
      cursorX: Math.round(cursorRef.current.x),
      cursorY: Math.round(cursorRef.current.y),
      touchMode,
      gesturePhase: touchGestureRef.current.phase,
      activePointers: touchGestureRef.current.pointers.size,
    };
    window.__rdView = {
      scale: view.scale,
      panX: view.panX,
      panY: view.panY,
      width: box.w,
      height: box.h,
    };
    window.__rdDiag = diagSummary;
  }, [phase, stats, rtt, touchMode, view.scale, view.panX, view.panY, box.w, box.h, diagSummary]);

  const resetDiagnostics = useCallback(() => {
    diagStateRef.current = createDiagnosticsState();
    diagSeqRef.current = 0;
    setDiagSummary(computeDiagnosticsSummary(diagStateRef.current, performance.now()));
  }, []);

  // Raw WebSocket ping-pong probing when Diagnostic Overlay is active during a session
  useEffect(() => {
    if ((!diagnosticsOpen && !qualityMonitor) || (phase !== 'live' && phase !== 'connecting')) {
      return;
    }

    const sendProbe = () => {
      const sock = wsRef.current;
      const now = performance.now();
      diagStateRef.current = sweepTimeouts(diagStateRef.current, now, PING_TIMEOUT_MS);
      if (sock && sock.readyState === WebSocket.OPEN) {
        const seq = diagSeqRef.current + 1;
        diagSeqRef.current = seq;
        diagStateRef.current = recordPingSent(diagStateRef.current, seq, now);
        sock.send(JSON.stringify({ type: 'ping', seq, clientTs: now }));
      }
      setDiagSummary(computeDiagnosticsSummary(diagStateRef.current, now, PING_TIMEOUT_MS));
    };

    sendProbe();
    const timer = window.setInterval(sendProbe, PING_INTERVAL_MS);
    return () => {
      window.clearInterval(timer);
    };
  }, [diagnosticsOpen, qualityMonitor, phase]);

  // Fetch /api/health on mount
  useEffect(() => {
    let active = true;
    fetch('/api/health')
      .then((res) => (res.ok ? res.json() : null))
      .then((data) => {
        if (active && data && typeof data.authRequired === 'boolean') {
          setAuthRequired(data.authRequired);
        }
      })
      .catch(() => {
        if (active) setAuthRequired(false);
      });
    return () => {
      active = false;
    };
  }, []);

  // ResizeObserver on stage + orientation flip reset to Fit
  useEffect(() => {
    const el = stageRef.current;
    if (!el) return;
    const update = () => {
      const w = el.clientWidth;
      const h = el.clientHeight;
      setBox((prev) => (prev.w === w && prev.h === h ? prev : { w, h }));
      if (w > 0 && h > 0) {
        const isLandscape = w > h;
        if (prevLandscapeRef.current !== null && prevLandscapeRef.current !== isLandscape) {
          setViewMode('fit');
          setZoom(1);
          setPan({ panX: 0, panY: 0 });
        }
        prevLandscapeRef.current = isLandscape;
      }
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const resetPageZoom = useCallback(() => {
    window.scrollTo(0, 0);
    const meta = document.querySelector<HTMLMetaElement>('meta[name="viewport"]');
    if (meta) {
      const original =
        'width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no, viewport-fit=cover';
      meta.setAttribute(
        'content',
        'width=device-width, initial-scale=1, minimum-scale=1, maximum-scale=1, user-scalable=no, viewport-fit=cover',
      );
      window.setTimeout(() => {
        meta.setAttribute('content', original);
        const vv = window.visualViewport;
        setPageZoomed(vv ? isPageZoomed(vv.scale) : false);
      }, 60);
    } else {
      const vv = window.visualViewport;
      setPageZoomed(vv ? isPageZoomed(vv.scale) : false);
    }
  }, []);

  // visualViewport keyboard inset (--kb) + Step-3 browser-zoom fallback detection
  useEffect(() => {
    const vv = window.visualViewport;
    if (!vv) return;
    let prevZoomed = false;
    let prevInset = -1;
    const syncViewport = () => {
      const inset = Math.round(keyboardInset(window.innerHeight, vv.height, vv.offsetTop));
      if (inset !== prevInset) {
        prevInset = inset;
        document.documentElement.style.setProperty('--kb', `${inset}px`);
      }
      const zoomed = isPageZoomed(vv.scale);
      if (zoomed !== prevZoomed) {
        setPageZoomed(zoomed);
        if (zoomed) {
          // Attempt automatic viewport scale restoration first; keep banner if browser retains zoom
          resetPageZoom();
        }
        prevZoomed = zoomed;
      }
    };
    syncViewport();
    vv.addEventListener('resize', syncViewport);
    vv.addEventListener('scroll', syncViewport);
    return () => {
      vv.removeEventListener('resize', syncViewport);
      vv.removeEventListener('scroll', syncViewport);
      document.documentElement.style.setProperty('--kb', '0px');
    };
  }, [resetPageZoom]);

  const focusDesktopCapture = useCallback(() => {
    if (window.matchMedia('(pointer: coarse)').matches) return;
    captureRef.current?.focus({ preventScroll: true });
  }, []);

  const closeChrome = useCallback(() => {
    chromeRef.current = { menu: false, dialog: false };
    setMenuOpen(false);
    setConnectDialog(false);
    focusDesktopCapture();
  }, [focusDesktopCapture]);

  const hideSoftKeyboard = useCallback(() => {
    setSoftKeys(false);
    focusDesktopCapture();
  }, [focusDesktopCapture]);

  const openSoftKeyboard = useCallback(() => {
    // iOS requires focus during the original tap to open the system keyboard.
    flushSync(() => {
      closeChrome();
      setSoftKeys(true);
    });
    softKeyboardInputRef.current?.focus({ preventScroll: true });
  }, [closeChrome]);

  const toggleSoftKeyboard = () => {
    if (softKeys) {
      closeChrome();
      hideSoftKeyboard();
    } else {
      openSoftKeyboard();
    }
  };

  const publishStatsSnapshot = useCallback((nowMs: number) => {
    const rates = computeFrameRates(frameStatsRef.current, nowMs);
    const decMetrics = decoderRef.current?.getMetrics();
    const acc = statAccRef.current;
    const decoded = decMetrics ? decMetrics.renderedCount : acc.decoded;
    const dropped = decMetrics ? decMetrics.droppedCount : acc.dropped;
    setStats((prev) => {
      const nextWidth = acc.width > 0 ? acc.width : prev.width;
      const nextHeight = acc.height > 0 ? acc.height : prev.height;
      const nextCodec = acc.lastCodec !== '—' ? acc.lastCodec : prev.codec;
      if (
        prev.codec === nextCodec &&
        prev.width === nextWidth &&
        prev.height === nextHeight &&
        prev.fps === rates.fps &&
        prev.rxFps === rates.rxFps &&
        prev.bitrate === rates.bitrate &&
        prev.delayMs === acc.lastDelayMs &&
        prev.decoded === decoded &&
        prev.dropped === dropped &&
        prev.streamState === rates.streamState
      ) {
        return prev;
      }
      return {
        codec: nextCodec,
        width: nextWidth,
        height: nextHeight,
        fps: rates.fps,
        rxFps: rates.rxFps,
        delayMs: acc.lastDelayMs,
        bitrate: rates.bitrate,
        decoded,
        dropped,
        streamState: rates.streamState,
      };
    });
  }, []);

  const ensureDecoder = useCallback(() => {
    if (!decoderRef.current) {
      decoderRef.current = new RemoteDecoder({
        onOutput: (w, h, renderedCount, nowMs) => {
          const sizeChanged = sizeRef.current.width !== w || sizeRef.current.height !== h;
          sizeRef.current = { width: w, height: h };
          statAccRef.current.width = w;
          statAccRef.current.height = h;
          statAccRef.current.decoded = renderedCount;
          recordRenderedFrame(frameStatsRef.current, nowMs);
          if (renderedCount === 1 || sizeChanged) {
            publishStatsSnapshot(nowMs);
          }
        },
        onError: (msg) => {
          setDecoderError(msg);
        },
        onNeedRefresh: () => {
          send({ type: 'refresh' });
        },
      });
    }
    return decoderRef.current;
  }, [publishStatsSnapshot, send]);

  const noteFrame = useCallback(
    (codecName: string, w: number, h: number, byteLen: number, delayMs: number) => {
      const now = performance.now();
      const firstOrCodecChange =
        frameStatsRef.current.totalRx === 0 ||
        statAccRef.current.lastCodec !== codecName ||
        (w > 0 && statAccRef.current.width !== w) ||
        (h > 0 && statAccRef.current.height !== h);

      recordRxFrame(frameStatsRef.current, now, byteLen);
      statAccRef.current.lastCodec = codecName;
      statAccRef.current.lastDelayMs = Math.max(0, Math.round(delayMs));
      if (w > 0) statAccRef.current.width = w;
      if (h > 0) statAccRef.current.height = h;

      if (firstOrCodecChange) {
        publishStatsSnapshot(now);
      }
    },
    [publishStatsSnapshot],
  );

  // Publish real-time sampling FPS, bitrate, and idle-screen decay at a steady 4Hz cadence
  useEffect(() => {
    if (phase !== 'live') return;
    const timer = window.setInterval(() => {
      publishStatsSnapshot(performance.now());
    }, 250);
    return () => window.clearInterval(timer);
  }, [phase, publishStatsSnapshot]);

  const connectSession = useCallback(
    async (req?: ConnectRequest, retry = false) => {
      const generation = ++connectGeneration.current;
      if (!retry) {
        setRestartState(null);
        reconnectRequest.current = null;
      }
      const activeMode = req?.mode ?? mode;
      const targetPeerId = (req?.peerId ?? peerId).replace(/\s+/g, '');
      const targetIdServer = (req?.idServer ?? idServer).trim();
      const targetRelayServer = (req?.relayServer ?? relayServer).trim();
      const targetServerKey = (req?.serverKey ?? serverKey).trim();
      const targetForceRelay = req?.forceRelay ?? forceRelay;
      const targetHost = (req?.host ?? host).trim();
      const targetPortStr = String(req?.port ?? port).trim() || '21118';
      const targetPort = Number(targetPortStr) || 21118;
      const targetPassword = req?.password ?? password;
      const shouldRemember = req?.remember ?? rememberPassword;
      const targetDeviceKey = deviceKeyFor({
        mode: activeMode,
        peerId: targetPeerId,
        host: targetHost,
        port: targetPortStr,
      });

      const wc = webCodecsStatus();
      if (!wc.ok) {
        setPhase('error');
        setDecoderError(wc.reason);
        setStatusDetail(null, wc.reason);
        setConnectDialog(true);
        return;
      }

      if (activeMode === 'direct' && !targetHost) {
        setPhase('error');
        setStatusDetail('detailMissingHost');
        return;
      }
      if (activeMode === 'id' && (!targetPeerId || !targetIdServer)) {
        setPhase('error');
        setStatusDetail('detailMissingId');
        return;
      }

      const rawSupport = await probeCodecs();
      if (generation !== connectGeneration.current) return;
      if (
        !rawSupport.vp8 &&
        !rawSupport.vp9 &&
        !rawSupport.h264 &&
        !rawSupport.h265 &&
        !rawSupport.av1
      ) {
        setPhase('error');
        setDecoderError(t(locale, 'detailNoCodecs'));
        setStatusDetail('detailNoCodecs');
        setConnectDialog(true);
        return;
      }
      const preferredCodec = displayPrefsRef.current.codec;
      const support = filterCodecSupport(rawSupport, preferredCodec);

      // Close previous socket & decoder cleanly
      if (wsRef.current) {
        wsRef.current.onclose = null;
        wsRef.current.onerror = null;
        wsRef.current.onmessage = null;
        wsRef.current.close();
        wsRef.current = null;
      }
      decoderRef.current?.close();
      frameStatsRef.current = createFrameStats();
      statAccRef.current = {
        width: 0,
        height: 0,
        decoded: 0,
        dropped: 0,
        lastDelayMs: 0,
        lastCodec: '—',
      };

      const initialQuality = resolveQualityLevel(
        displayPrefsRef.current.qualityPreset,
        displayPrefsRef.current.customBitrate,
      );
      const initialFps = resolveTargetFps(
        displayPrefsRef.current.qualityPreset,
        displayPrefsRef.current.customFps,
      );

      setPeer(null);
      setResolutionOptions(null);
      setCursorShape(null);
      setCursorEmbedded(false);
      placeCursor(sizeRef.current.width / 2, sizeRef.current.height / 2);
      setStats(EMPTY_STATS);
      setRtt(0);
      resetDiagnostics();
      setDecoderError('');
      setMenuOpen(false);
      setSoftKeys(false);
      setPhase('connecting');
      setStatusDetail('detailOpeningSocket');

      const sock = new WebSocket(wsUrl());
      sock.binaryType = 'arraybuffer';
      wsRef.current = sock;
      let receivedTerminalError = false;
      let sessionDisplays: PeerMessage['displays'] = [];
      let activeDisplay = 0;
      let cursorDisplayOrigin = { x: 0, y: 0 };

      sock.onopen = () => {
        if (wsRef.current !== sock) return;
        setStatusDetail('detailAuthenticating');
        if (activeMode === 'id') {
          sock.send(
            JSON.stringify({
              type: 'connect',
              mode: 'id',
              peerId: targetPeerId,
              idServer: targetIdServer,
              relayServer: targetRelayServer,
              key: targetServerKey,
              forceRelay: targetForceRelay,
              password: targetPassword,
              token: gatewayToken,
              codecs: support,
              quality: initialQuality,
              fps: initialFps,
              codec: preferredCodec,
            }),
          );
        } else {
          sock.send(
            JSON.stringify({
              type: 'connect',
              mode: 'direct',
              host: targetHost,
              port: targetPort,
              password: targetPassword,
              token: gatewayToken,
              codecs: support,
              quality: initialQuality,
              fps: initialFps,
              codec: preferredCodec,
            }),
          );
        }
      };

      sock.onmessage = (ev) => {
        if (wsRef.current !== sock) return;
        if (ev.data instanceof ArrayBuffer) {
          const packet = decodeFrame(ev.data);
          if (!packet) return;
          ensureDecoder().push(packet, canvasRef.current);
          noteFrame(
            packet.codecName,
            packet.width,
            packet.height,
            packet.data.byteLength,
            Date.now() - packet.gatewayUnixMs,
          );
          return;
        }

        if (typeof ev.data !== 'string') return;
        let msg: ServerMessage;
        try {
          msg = JSON.parse(ev.data) as ServerMessage;
        } catch {
          return;
        }

        switch (msg.type) {
          case 'status':
            if (msg.state === 'live') {
              reconnectRequest.current = {
                mode: activeMode, peerId: targetPeerId, idServer: targetIdServer, relayServer: targetRelayServer,
                serverKey: targetServerKey, forceRelay: targetForceRelay, host: targetHost, port: targetPortStr,
                password: targetPassword, remember: shouldRemember,
              };
              setRestartState(null);
              phaseRef.current = 'live';
              setPhase('live');
              setConnectDialog(false);
              setMenuOpen(false);
              setStatusDetail(
                msg.detail === 'encrypted' ? 'detailEncryptedSession' : 'linkPlain',
              );
              if (shouldRemember && targetPassword) {
                setSavedPasswords((prev) => setSavedPassword(prev, targetDeviceKey, targetPassword));
              } else if (!shouldRemember) {
                setSavedPasswords((prev) => removeSavedPassword(prev, targetDeviceKey));
              }
              focusDesktopCapture();
            } else if (msg.state === 'connecting') {
              phaseRef.current = 'connecting';
              setPhase('connecting');
              if (msg.detail) {
                setStatusDetail(null, msg.detail);
              } else {
                setStatusDetail('connecting');
              }
            } else if (msg.state === 'error') {
              if (msg.detail && /password|token|two.factor/i.test(msg.detail)) setRestartState(null);
              receivedTerminalError = true;
              phaseRef.current = 'error';
              setPhase('error');
              setConnectDialog(true);
              setMenuOpen(false);
              if (msg.detail && /password/i.test(msg.detail)) {
                // Clear stale remembered password so the next attempt prompts cleanly
                setSavedPasswords((prev) => removeSavedPassword(prev, targetDeviceKey));
              }
              if (msg.detail && /token/i.test(msg.detail)) {
                setAuthRequired(true);
                setStatusDetail('detailTokenError');
              } else if (msg.detail) {
                setStatusDetail(null, msg.detail);
              } else {
                setStatusDetail('detailHostRejected');
              }
            } else if (msg.state === 'closed') {
              phaseRef.current = 'idle';
              setPhase('idle');
              setStatusDetail('detailDisconnected');
            }
            break;

          case 'peer': {
            sessionDisplays = msg.displays;
            activeDisplay = msg.currentDisplay;
            setPeer(msg);
            setDisplay(msg.currentDisplay);
            const chosen = msg.displays[msg.currentDisplay] ?? msg.displays[0];
            cursorDisplayOrigin = { x: chosen?.x ?? 0, y: chosen?.y ?? 0 };
            setCursorEmbedded(Boolean(chosen?.cursorEmbedded));
            if (chosen && chosen.width > 0 && chosen.height > 0) {
              sizeRef.current = { width: chosen.width, height: chosen.height };
              setStats((prev) => ({ ...prev, width: chosen.width, height: chosen.height }));
              placeCursor(chosen.width / 2, chosen.height / 2);
            }
            setDevices((prev) =>
              upsertRecentDevice(
                prev,
                activeMode === 'id'
                  ? {
                      mode: 'id',
                      peerId: peerId.replace(/\s+/g, ''),
                      idServer: idServer.trim(),
                      relayServer: relayServer.trim(),
                      serverKey: serverKey.trim(),
                      forceRelay,
                      hostname: msg.hostname || '',
                      platform: msg.platform || '',
                    }
                  : {
                      mode: 'direct',
                      host: host.trim(),
                      port: port.trim() || '21118',
                      hostname: msg.hostname || '',
                      platform: msg.platform || '',
                    },
                Date.now(),
              ),
            );
            break;
          }

          case 'restartPermission':
            setPeer(previous => previous ? { ...previous, restartAllowed: msg.allowed } : previous);
            break;
          case 'restartResult':
            if (!msg.sent) {
              setRestartState(null);
              setDecoderError(t(locale, 'restartFailed'));
              setMobilePanel('actions');
              setMenuOpen(true);
            }
            break;
          case 'audioDenied':
            stopAudio();
            setAudioIssue('audioDenied');
            break;
          case 'audioFormat':
            audioRef.current?.configure(msg.sampleRate, msg.channels);
            break;
          case 'audioFrame':
            audioRef.current?.push(msg.packet);
            break;
          case 'focusedDisplay':
            if (displayPrefsRef.current.followWindow && msg.display !== activeDisplay &&
                sessionDisplays.some(d => d.index === msg.display && d.online !== false)) {
              send({ type: 'switchDisplay', display: msg.display });
            }
            break;

          case 'display':
            activeDisplay = msg.display;
            setDisplay(msg.display);
            cursorDisplayOrigin = { x: msg.x ?? 0, y: msg.y ?? 0 };
            setCursorEmbedded(Boolean(msg.cursorEmbedded));
            if (msg.width > 0 && msg.height > 0) {
              sizeRef.current = { width: msg.width, height: msg.height };
              setStats((prev) => ({ ...prev, width: msg.width, height: msg.height }));
              setViewMode('fit');
              setZoom(1);
              setPan({ panX: 0, panY: 0 });
            }
            break;

          case 'cursor': {
            const x = msg.x - cursorDisplayOrigin.x;
            const y = msg.y - cursorDisplayOrigin.y;
            const { width, height } = sizeRef.current;
            if (x >= 0 && x < width && y >= 0 && y < height) {
              placeCursor(x, y);
            } else if (cursorElRef.current) {
              cursorElRef.current.style.visibility = 'hidden';
            }
            break;
          }

          case 'resolutions':
            setResolutionOptions(msg);
            break;

          case 'cursorShape': {
            try {
              const shape = msg.shape;
              if (!shape) {
                setCursorShape(null);
                break;
              }
              const { width, height, hotX, hotY, rgba } = shape;
              const pixels = Uint8ClampedArray.from(atob(rgba), (char) => char.charCodeAt(0));
              setCursorShape({ pixels: new ImageData(pixels, width, height), hotX, hotY });
            } catch {
              setCursorShape(null);
            }
            break;
          }

          case 'clipboard':
            if (!displayPrefsRef.current.disableClipboard) {
              setRemoteClipboard(msg.text);
            }
            break;

          case 'rtt':
            setRtt(msg.ms);
            break;

          case 'pong': {
            const now = performance.now();
            diagStateRef.current = recordPongReceived(diagStateRef.current, msg.seq, now);
            setDiagSummary(computeDiagnosticsSummary(diagStateRef.current, now, PING_TIMEOUT_MS));
            break;
          }

          case 'error':
            setDecoderError(msg.message);
            setStatusDetail(null, msg.message);
            if (phaseRef.current === 'connecting') {
              if (/password|token|two.factor/i.test(msg.message)) setRestartState(null);
              receivedTerminalError = true;
              phaseRef.current = 'error';
              setPhase('error');
              setConnectDialog(true);
            }
            break;

          default:
            break;
        }
      };

      sock.onerror = () => {
        if (wsRef.current !== sock || receivedTerminalError) return;
        phaseRef.current = 'error';
        setPhase('error');
        setConnectDialog(true);
        setStatusDetail('detailGatewayError');
      };

      sock.onclose = () => {
        if (wsRef.current !== sock) return;
        wsRef.current = null;
        if (receivedTerminalError || phaseRef.current === 'error') {
          return;
        }
        if (phaseRef.current === 'connecting') {
          phaseRef.current = 'error';
          setPhase('error');
          setConnectDialog(true);
          setStatusDetail('detailGatewayClosed');
        } else {
          phaseRef.current = 'idle';
          setPhase('idle');
        }
      };
    },
    [
      locale,
      mode,
      host,
      port,
      password,
      rememberPassword,
      peerId,
      idServer,
      relayServer,
      serverKey,
      forceRelay,
      gatewayToken,
      ensureDecoder,
      noteFrame,
      placeCursor,
      resetDiagnostics,
      setStatusDetail,
    ],
  );

  useEffect(() => {
    if (!restartState) {
      if (phase === 'idle' || phase === 'error') reconnectRequest.current = null;
      return;
    }
    if (restartState === 'requested' && phase !== 'live') {
      setRestartState('waiting');
      return;
    }
    if (restartState === 'waiting' && (phase === 'idle' || phase === 'error')) {
      const timer = window.setTimeout(() => {
        if (reconnectRequest.current && Date.now() < restartDeadline.current) {
          void connectSession(reconnectRequest.current, true);
        }
      }, 5000);
      return () => window.clearTimeout(timer);
    }
  }, [restartState, phase, connectSession]);

  useEffect(() => {
    if (!restartState) return;
    const timer = window.setTimeout(() => {
      setRestartState(null);
      setStatusDetail(restartState === 'requested' ? 'restartFailed' : 'retryEnded');
      if (restartState === 'requested') {
        setDecoderError(t(locale, 'restartFailed'));
        setMobilePanel('actions');
        setMenuOpen(true);
      }
    }, restartState === 'requested' ? 45000 : Math.max(0, restartDeadline.current - Date.now()));
    return () => window.clearTimeout(timer);
  }, [restartState, locale, setStatusDetail]);

  const requestRestart = () => {
    if (restartState || !peer?.restartAllowed || wsRef.current?.readyState !== WebSocket.OPEN) return;
    if (!window.confirm(t(locale, 'confirmRestart'))) return;
    restartDeadline.current = Date.now() + 180000;
    setRestartState('requested');
    setMenuOpen(false);
    send({ type: 'restart' });
  };

  // Disconnect cleanly and return to idle state

  const disconnect = useCallback(() => {
    connectGeneration.current++;
    reconnectRequest.current = null;
    setRestartState(null);
    finishPendingClick();
    clearLongPress();
    if (phaseRef.current === 'live' && displayPrefsRef.current.lockAfterSessionEnd) {
      send({ type: 'key', action: 'down', control: 'LockScreen' });
      send({ type: 'key', action: 'up', control: 'LockScreen' });
    }
    send({ type: 'disconnect' });
    if (wsRef.current) {
      wsRef.current.onclose = null;
      wsRef.current.close();
      wsRef.current = null;
    }
    decoderRef.current?.close();
    setMenuOpen(false);
    setToolbarExpanded(false);
    setConnectDialog(false);
    setSoftKeys(false);
    resetDiagnostics();
    setPhase('idle');
    setStatusDetail('detailDisconnected');
  }, [clearLongPress, finishPendingClick, resetDiagnostics, send, setStatusDetail]);

  const handleDismissError = useCallback(() => {
    setDecoderError('');
    if (phaseRef.current === 'error') {
      setPhase('idle');
      setStatusDetail('detailIdle');
    }
  }, [setStatusDetail]);

  const handleUpdateDisplayPrefs = useCallback(
    (patch: Partial<DisplayPrefs>) => {
      setDisplayPrefs((prev) => {
        const next: DisplayPrefs = { ...prev, ...patch };
        persistDisplayPrefs(next);

        if (patch.viewStyle) {
          setViewMode(patch.viewStyle);
          if (patch.viewStyle === 'fit') {
            setZoom(1);
            setPan({ panX: 0, panY: 0 });
          } else {
            setPan({
              panX: (box.w - remoteW) / 2,
              panY: (box.h - remoteH) / 2,
            });
          }
        }

        if (
          patch.qualityPreset !== undefined ||
          patch.customBitrate !== undefined ||
          patch.customFps !== undefined ||
          patch.codec !== undefined
        ) {
          const nextQ = resolveQualityLevel(next.qualityPreset, next.customBitrate);
          const nextFps = resolveTargetFps(next.qualityPreset, next.customFps);
          setQuality(nextQ);
          setTargetFps(nextFps);
          send({ type: 'quality', quality: nextQ, fps: nextFps, codec: next.codec });
        }

        return next;
      });
    },
    [box.w, box.h, remoteW, remoteH, send],
  );

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      finishPendingClick();
      clearLongPress();
      if (moveRafRef.current !== null) cancelAnimationFrame(moveRafRef.current);
      wsRef.current?.close();
      decoderRef.current?.close();
    };
  }, [clearLongPress, finishPendingClick]);

  // Scale helper (synchronously updates viewRef so rapid 120Hz pinch/wheel events never read stale state)
  const setScaleAround = useCallback(
    (nextScale: number, anchorX: number, anchorY: number) => {
      const next = computeNextScaleState(
        box.w,
        box.h,
        remoteW,
        remoteH,
        viewRef.current,
        nextScale,
        anchorX,
        anchorY,
        panClampOptions,
      );
      viewRef.current = {
        scale: next.scale,
        panX: next.pan.panX,
        panY: next.pan.panY,
      };
      setViewMode(next.viewMode);
      setZoom(next.zoom);
      setPan(next.pan);
    },
    [box.w, box.h, remoteW, remoteH, panClampOptions],
  );

  const applyPinchStep = useCallback(
    (
      prevMidX: number,
      prevMidY: number,
      prevDist: number,
      nextMidX: number,
      nextMidY: number,
      nextDist: number,
    ) => {
      const next = computePinchStep(
        box.w,
        box.h,
        remoteW,
        remoteH,
        viewRef.current,
        prevMidX,
        prevMidY,
        prevDist,
        nextMidX,
        nextMidY,
        nextDist,
        panClampOptions,
      );
      viewRef.current = {
        scale: next.scale,
        panX: next.pan.panX,
        panY: next.pan.panY,
      };
      setViewMode(next.viewMode);
      setZoom(next.zoom);
      setPan(next.pan);
    },
    [box.w, box.h, remoteW, remoteH, panClampOptions],
  );
  const applyPinchStepRef = useRef(applyPinchStep);
  applyPinchStepRef.current = applyPinchStep;

  const panBy = useCallback(
    (dx: number, dy: number) => {
      const cur = viewRef.current;
      const curFit = fitScale(box.w, box.h, remoteW, remoteH);
      const nextPan = clampPan(
        box.w,
        box.h,
        remoteW,
        remoteH,
        cur.scale,
        cur.panX + dx,
        cur.panY + dy,
        panClampOptions,
      );
      viewRef.current = { ...cur, panX: nextPan.panX, panY: nextPan.panY };
      setPan(nextPan);
      if (viewMode === 'fit') {
        setViewMode('custom');
        setZoom(curFit > 0 ? cur.scale / curFit : 1);
      }
    },
    [box.w, box.h, remoteW, remoteH, viewMode, panClampOptions],
  );

  followCursorRef.current = (x, y) => {
    const view = viewRef.current;
    const point = mapFromRemote(x, y, view);
    const visibleHeight = Math.min(box.h, window.visualViewport?.height ?? box.h) - panClampOptions.bottomInset;
    if (box.w <= 0 || visibleHeight <= 0) return;
    const marginX = Math.min(32, box.w / 4);
    const marginY = Math.min(32, visibleHeight / 4);
    const dx = remoteW * view.scale > box.w
      ? Math.min(box.w - marginX, Math.max(marginX, point.x)) - point.x : 0;
    const dy = remoteH * view.scale > visibleHeight
      ? Math.min(visibleHeight - marginY, Math.max(marginY, point.y)) - point.y : 0;
    if (dx || dy) panBy(dx, dy);
  };

  // Forward text efficiently: single ASCII chars as native keystrokes, CJK/multi-char as batched seq
  const sendText = useCallback(
    (text: string) => {
      if (!text) return;
      if (text === ' ') {
        send({ type: 'key', action: 'down', control: 'Space' });
        send({ type: 'key', action: 'up', control: 'Space' });
        return;
      }
      if (isPrintableAscii(text)) {
        send({ type: 'key', action: 'down', chr: text });
        send({ type: 'key', action: 'up', chr: text });
        return;
      }
      const chars = Array.from(text);
      const CHUNK = 64;
      for (let i = 0; i < chars.length; i += CHUNK) {
        send({ type: 'text', text: chars.slice(i, i + CHUNK).join('') });
      }
    },
    [send],
  );

  const tapControl = useCallback(
    (control: string) => {
      if (chromeRef.current.menu || chromeRef.current.dialog) return;
      send({ type: 'key', action: 'down', control });
      send({ type: 'key', action: 'up', control });
    },
    [send],
  );

  const sendChord = useCallback(
    (parts: Array<{ control?: string; chr?: string }>) => {
      if (chromeRef.current.menu || chromeRef.current.dialog) return;
      const modifiers = parts
        .map((p) => p.control)
        .filter((c): c is string => Boolean(c && isModifier(c)));

      for (const part of parts) {
        if (part.control) {
          send({ type: 'key', action: 'down', control: part.control, modifiers });
        } else if (part.chr) {
          send({ type: 'key', action: 'down', chr: part.chr, modifiers });
        }
      }
      for (let i = parts.length - 1; i >= 0; i--) {
        const part = parts[i];
        if (part.chr) {
          send({ type: 'key', action: 'up', chr: part.chr, modifiers });
        } else if (part.control) {
          send({ type: 'key', action: 'up', control: part.control, modifiers });
        }
      }
    },
    [send],
  );

  const handleQuickKey = useCallback(
    (name: 'Esc' | 'Tab' | 'Ctrl+C' | 'Ctrl+V' | 'Ctrl+Alt+Del' | 'Lock') => {
      closeChrome();
      if (name === 'Esc') tapControl('Escape');
      else if (name === 'Tab') tapControl('Tab');
      else if (name === 'Ctrl+C') sendChord([{ control: 'Control' }, { chr: 'c' }]);
      else if (name === 'Ctrl+V') sendChord([{ control: 'Control' }, { chr: 'v' }]);
      else if (name === 'Ctrl+Alt+Del') tapControl('CtrlAltDel');
      else if (name === 'Lock') tapControl('LockScreen');
    },
    [closeChrome, sendChord, tapControl],
  );

  // Global physical keyboard listener (capture phase + full host-browser shortcut isolation)
  useEffect(() => {
    const isInsideChromeInput = (target: EventTarget | null): boolean => {
      if (!(target instanceof Element)) return false;
      return Boolean(
        target.closest('[data-testid="connection-panel"]') ||
          target.closest('[data-testid="session-menu"]') ||
          target.closest('[data-testid="softbar"]'),
      );
    };

    const onKeyDown = (e: KeyboardEvent) => {
      if (chromeRef.current.menu || chromeRef.current.dialog) {
        if (e.key === 'Escape' && phaseRef.current === 'live') {
          e.preventDefault();
          closeChrome();
          return;
        }
        if (isInsideChromeInput(e.target)) {
          return;
        }
        e.preventDefault();
        return;
      }

      if (
        !shouldIsolateKeyboardEvent({
          phase: phaseRef.current,
          chromeOpen: chromeRef.current.menu || chromeRef.current.dialog,
          insideChromeInput: isInsideChromeInput(e.target),
        })
      ) {
        return;
      }

      // Let native OS IME (Chinese/Japanese/Korean) compose smoothly inside the hidden capture input
      if (captureComposingRef.current || isImeEvent(e)) {
        return;
      }

      // Prevent host browser shortcuts (Ctrl+R/F/P/S/+/-, Tab, Space, Backspace, Alt, F1-F12, repeats)
      e.preventDefault();
      e.stopPropagation();

      if (displayPrefsRef.current.viewOnly) {
        return;
      }

      const mapped = mapPhysicalKeyDown(e);
      if (mapped) {
        if (mapped.type === 'key' && mapped.chr && e.code) {
          pressedChrByCodeRef.current.set(e.code, mapped.chr);
        }
        send(mapped);
      }
    };

    const onKeyUp = (e: KeyboardEvent) => {
      if (
        !shouldIsolateKeyboardEvent({
          phase: phaseRef.current,
          chromeOpen: chromeRef.current.menu || chromeRef.current.dialog,
          insideChromeInput: isInsideChromeInput(e.target),
        })
      ) {
        return;
      }

      if (
        captureComposingRef.current ||
        isImeEvent(e) ||
        performance.now() - lastCompositionEndAtRef.current < 80
      ) {
        return;
      }

      // Prevent Alt/Meta keyup from focusing browser chrome menus
      e.preventDefault();
      e.stopPropagation();

      if (displayPrefsRef.current.viewOnly) {
        return;
      }

      const trackedChr = e.code ? pressedChrByCodeRef.current.get(e.code) : undefined;
      if (e.code) {
        pressedChrByCodeRef.current.delete(e.code);
      }
      const mapped = mapPhysicalKeyUp(e, trackedChr);
      if (mapped && mapped.type === 'key') {
        send(mapped);
      }
    };

    const onWindowBlur = () => {
      captureComposingRef.current = false;
      pressedChrByCodeRef.current.clear();
    };

    window.addEventListener('keydown', onKeyDown, true);
    window.addEventListener('keyup', onKeyUp, true);
    window.addEventListener('blur', onWindowBlur);
    return () => {
      window.removeEventListener('keydown', onKeyDown, true);
      window.removeEventListener('keyup', onKeyUp, true);
      window.removeEventListener('blur', onWindowBlur);
    };
  }, [closeChrome, send]);

  // Helper: map stage client coords to remote coords
  const stageLocalPoint = useCallback((clientX: number, clientY: number) => {
    const rect = stageRef.current?.getBoundingClientRect();
    if (!rect) return { x: clientX, y: clientY };

    return { x: clientX - rect.left, y: clientY - rect.top };
  }, []);

  const remoteFromLocal = useCallback(
    (localX: number, localY: number) =>
      mapToRemote(localX, localY, remoteW, remoteH, viewRef.current),
    [remoteW, remoteH],
  );

  const remoteFromLocalClamped = useCallback(
    (localX: number, localY: number) =>
      mapToRemoteClamped(localX, localY, remoteW, remoteH, viewRef.current),
    [remoteW, remoteH],
  );

  const syncGestureDebug = useCallback((state: TouchGestureState) => {
    if (window.__rd) {
      window.__rd.gesturePhase = state.phase;
      window.__rd.activePointers = state.pointers.size;
    }
  }, []);

  const runTouchEffectsRef = useRef<(effects: TouchGestureEffect[]) => void>(() => {});

  const runTouchEffects = useCallback(
    (effects: TouchGestureEffect[]) => {
      for (const ef of effects) {
        switch (ef.type) {
          case 'cancel-long-press':
            clearLongPress();
            break;
          case 'arm-long-press': {
            clearLongPress();
            const targetId = ef.pointerId;
            longTimerRef.current = window.setTimeout(() => {
              longTimerRef.current = null;
              const next = gestureLongPressTimeout(touchGestureRef.current, targetId);
              touchGestureRef.current = next.state;
              syncGestureDebug(next.state);
              runTouchEffectsRef.current(next.effects);
            }, LONG_PRESS_MS);
            break;
          }
          case 'long-press': {
            if (!remoteInputRef.current || displayPrefsRef.current.viewOnly) break;
            const pos = touchModeRef.current
              ? remoteFromLocal(ef.x, ef.y)
              : cursorPoint();
            if (pos) {
              clickButton('right', pos.x, pos.y);
            }
            break;
          }
          case 'start-drag-lock': {
            if (!remoteInputRef.current || displayPrefsRef.current.viewOnly) break;
            finishPendingClick();
            const pt = cursorPoint();
            send({ type: 'mouse', action: 'move', x: pt.x, y: pt.y });
            send({ type: 'mouse', action: 'down', button: 'left', x: pt.x, y: pt.y });
            break;
          }
          case 'start-touch-drag': {
            if (!remoteInputRef.current || displayPrefsRef.current.viewOnly) break;
            finishPendingClick();
            const startPos = remoteFromLocalClamped(ef.x, ef.y);
            if (startPos) {
              placeCursor(startPos.x, startPos.y);
              send({ type: 'mouse', action: 'move', x: startPos.x, y: startPos.y });
              send({ type: 'mouse', action: 'down', button: 'left', x: startPos.x, y: startPos.y });
            }
            break;
          }
          case 'move-touch-drag': {
            if (!remoteInputRef.current || displayPrefsRef.current.viewOnly) break;
            const curPos = remoteFromLocalClamped(ef.x, ef.y);
            if (curPos) {
              placeCursor(curPos.x, curPos.y);
              queueMove(curPos);
            }
            break;
          }
          case 'end-drag':
          case 'release-stuck-drag': {
            flushMove();
            if (displayPrefsRef.current.viewOnly) break;
            const pos = touchModeRef.current
              ? remoteFromLocalClamped(ef.x, ef.y)
              : cursorPoint();
            if (pos) {
              send({ type: 'mouse', action: 'up', button: 'left', x: pos.x, y: pos.y });
            }
            break;
          }
          case 'cursor-move': {
            const scale = Math.max(0.01, viewRef.current.scale);
            placeCursor(cursorRef.current.x + ef.dx / scale, cursorRef.current.y + ef.dy / scale);
            if (remoteInputRef.current && !displayPrefsRef.current.viewOnly) {
              queueMove(cursorPoint());
            }
            break;
          }
          case 'tap-click': {
            if (!remoteInputRef.current || displayPrefsRef.current.viewOnly) break;
            const pos = touchModeRef.current
              ? remoteFromLocal(ef.x, ef.y)
              : cursorPoint();
            if (pos) {
              const btn = rightArmedRef.current ? 'right' : 'left';
              if (rightArmedRef.current) setRightArmed(false);
              const atCursor = Math.hypot(pos.x - cursorRef.current.x, pos.y - cursorRef.current.y) <= 8;
              clickButton(btn, pos.x, pos.y);
              if (btn === 'left' && !displayPrefsRef.current.swapMouseButtons &&
                  atCursor && textCursor && !softKeys) openSoftKeyboard();
            }
            break;
          }
          case 'two-finger-tap': {
            if (!remoteInputRef.current || displayPrefsRef.current.viewOnly) break;
            const pos = touchModeRef.current
              ? remoteFromLocal(ef.x, ef.y)
              : cursorPoint();
            if (pos) {
              clickButton('right', pos.x, pos.y);
            }
            break;
          }
          case 'pan':
            panBy(ef.dx, ef.dy);
            break;
          case 'pinch':
            applyPinchStepRef.current(
              ef.prevMidX,
              ef.prevMidY,
              ef.prevDist,
              ef.nextMidX,
              ef.nextMidY,
              ef.nextDist,
            );
            break;
          case 'three-finger-scroll': {
            if (!remoteInputRef.current || displayPrefsRef.current.viewOnly) break;
            const clamped = Math.max(
              -MAX_WHEEL_TICKS_PER_EVENT,
              Math.min(MAX_WHEEL_TICKS_PER_EVENT, ef.ticks),
            );
            const dir = applyWheelDirection(clamped, displayPrefsRef.current.reverseMouseWheel);
            if (dir !== 0) {
              send({ type: 'mouse', action: 'wheel', x: 0, y: dir });
            }
            break;
          }
        }
      }
    },
    [
      openSoftKeyboard,
      softKeys,
      textCursor,
      clearLongPress,
      clickButton,
      cursorPoint,
      finishPendingClick,
      flushMove,
      panBy,
      placeCursor,
      queueMove,
      remoteFromLocal,
      remoteFromLocalClamped,
      send,
      syncGestureDebug,
    ],
  );
  runTouchEffectsRef.current = runTouchEffects;

  const resetAllGestures = useCallback(() => {
    if (zeroTouchTimerRef.current !== null) {
      window.clearTimeout(zeroTouchTimerRef.current);
      zeroTouchTimerRef.current = null;
    }
    touchFallbackPinchRef.current = null;
    wheelAccumRef.current = { x: 0, y: 0 };

    const next = resetTouchGestureState(touchGestureRef.current);
    touchGestureRef.current = next.state;
    syncGestureDebug(next.state);
    runTouchEffectsRef.current(next.effects);

    const drag = desktopDragRef.current;
    if (drag) {
      desktopDragRef.current = null;
      if (drag.mode === 'remote' && !displayPrefsRef.current.viewOnly) {
        cancelQueuedMove();
        const pos = remoteFromLocalClamped(drag.lastX, drag.lastY);
        if (pos) {
          send({ type: 'mouse', action: 'up', button: drag.button, x: pos.x, y: pos.y });
        }
      }
    }
  }, [cancelQueuedMove, remoteFromLocalClamped, send, syncGestureDebug]);

  const resetAllGesturesRef = useRef(resetAllGestures);
  resetAllGesturesRef.current = resetAllGestures;

  // Cleanly reset active gestures whenever remote input pauses (menu/dialog/disconnect) or mode switches
  useEffect(() => {
    if (!remoteInput) {
      resetAllGestures();
    }
  }, [remoteInput, resetAllGestures]);

  useEffect(() => {
    resetAllGestures();
  }, [touchMode, hand, resetAllGestures]);

  // Pointer handlers on stage
  const handlePointerDown = (e: React.PointerEvent<HTMLElement>) => {
    if (!remoteInput) return;
    // Ignore events originating from interactive overlays/chrome inside the stage
    if (e.target !== stageRef.current && e.target !== canvasRef.current) return;

    if (toolbarExpanded && !toolbarPinned) {
      setToolbarExpanded(false);
    }

    const isTouch = e.pointerType === 'touch';
    // Ignore browser-synthesized compatibility mouse events right after touch
    if (!isTouch && Date.now() - lastTouchAtRef.current < 450) {
      return;
    }

    const local = stageLocalPoint(e.clientX, e.clientY);

    if (!isTouch) {
      // Desktop mouse interaction
      setVirtualCursorVisible(false);

      // Middle-button drag, Pan toggle, or Alt+LeftDrag pans the viewport
      const shouldPan = hand || e.button === 1 || (e.button === 0 && e.altKey);
      if (shouldPan) {
        try {
          e.currentTarget.setPointerCapture(e.pointerId);
        } catch {
          // Ignore NotFoundError on synthetic or already-lifted pointers
        }
        desktopDragRef.current = {
          pointerId: e.pointerId,
          mode: 'pan',
          button: 'left',
          lastX: local.x,
          lastY: local.y,
        };
        return;
      }

      if (displayPrefsRef.current.viewOnly) return;

      const pos = remoteFromLocal(local.x, local.y);
      if (!pos) return;
      const rawBtn: 'left' | 'right' | 'middle' = rightArmed
        ? 'right'
        : e.button === 2
          ? 'right'
          : e.button === 1
            ? 'middle'
            : 'left';
      const btn = applyMouseButtonSwap(rawBtn, displayPrefsRef.current.swapMouseButtons);
      if (rightArmed) setRightArmed(false);
      try {
        e.currentTarget.setPointerCapture(e.pointerId);
      } catch {
        // Ignore NotFoundError on synthetic or already-lifted pointers
      }
      desktopDragRef.current = {
        pointerId: e.pointerId,
        mode: 'remote',
        button: btn,
        lastX: local.x,
        lastY: local.y,
      };
      // Arm hidden capture input on desktop clicks so OS IME (Chinese/CJK) works smoothly in remote input boxes
      focusDesktopCapture();
      // Always cancel stale rAF moves and sync cursor position BEFORE mouse down so window drags hit the exact titlebar
      cancelQueuedMove();
      send({ type: 'mouse', action: 'move', x: pos.x, y: pos.y });
      send({ type: 'mouse', action: 'down', button: btn, x: pos.x, y: pos.y });
      return;
    }

    // Touch interaction driven strictly by deterministic TouchGestureState FSM
    e.preventDefault();
    const now = Date.now();
    lastTouchAtRef.current = now;
    if (zeroTouchTimerRef.current !== null) {
      window.clearTimeout(zeroTouchTimerRef.current);
      zeroTouchTimerRef.current = null;
    }
    if (!touchMode) {
      setVirtualCursorVisible(true);
    }

    try {
      e.currentTarget.setPointerCapture(e.pointerId);
    } catch {
      // Ignore NotFoundError on synthetic or rapid multi-touch pointers
    }

    const liveTouchCount =
      now - nativeTouchInfoRef.current.updatedAt <= 120
        ? nativeTouchInfoRef.current.count
        : null;

    const next = gesturePointerDown(touchGestureRef.current, e.pointerId, local.x, local.y, {
      mode: touchMode ? 'touch' : 'mouse',
      hand,
      now,
      isPrimary: e.isPrimary,
      liveTouchCount,
    });
    touchGestureRef.current = next.state;
    syncGestureDebug(next.state);
    runTouchEffects(next.effects);
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLElement>) => {
    if (!remoteInput) return;

    const isTouch = e.pointerType === 'touch';
    if (!isTouch && Date.now() - lastTouchAtRef.current < 450) {
      return;
    }

    const local = stageLocalPoint(e.clientX, e.clientY);

    if (!isTouch) {
      const drag = desktopDragRef.current;
      if (drag && drag.pointerId === e.pointerId && drag.mode === 'pan') {
        const dx = local.x - drag.lastX;
        const dy = local.y - drag.lastY;
        drag.lastX = local.x;
        drag.lastY = local.y;
        panBy(dx, dy);
        return;
      }

      if (displayPrefsRef.current.viewOnly) return;

      const isRemoteDrag = Boolean(drag && drag.pointerId === e.pointerId && drag.mode === 'remote');
      if (isRemoteDrag && drag) {
        drag.lastX = local.x;
        drag.lastY = local.y;
      }
      const pos = isRemoteDrag
        ? remoteFromLocalClamped(local.x, local.y)
        : remoteFromLocal(local.x, local.y);
      if (pos) {
        if (isRemoteDrag) {
          cancelQueuedMove();
          send({ type: 'mouse', action: 'move', x: pos.x, y: pos.y });
        } else {
          queueMove(pos);
        }
      }
      return;
    }

    if (!touchGestureRef.current.pointers.has(e.pointerId)) return;
    const now = Date.now();
    lastTouchAtRef.current = now;

    const next = gesturePointerMove(touchGestureRef.current, e.pointerId, local.x, local.y, {
      mode: touchMode ? 'touch' : 'mouse',
      now,
    });
    touchGestureRef.current = next.state;
    syncGestureDebug(next.state);
    runTouchEffects(next.effects);
  };

  const handlePointerUpOrCancel = (e: React.PointerEvent<HTMLElement>) => {
    const isTouch = e.pointerType === 'touch';
    if (!isTouch && Date.now() - lastTouchAtRef.current < 450) {
      return;
    }

    const local = stageLocalPoint(e.clientX, e.clientY);

    if (!isTouch) {
      const drag = desktopDragRef.current;
      if (drag && drag.pointerId === e.pointerId) {
        desktopDragRef.current = null;
        if (drag.mode === 'remote' && !displayPrefsRef.current.viewOnly) {
          cancelQueuedMove();
          const pos = remoteFromLocalClamped(local.x, local.y);
          if (pos) {
            send({ type: 'mouse', action: 'move', x: pos.x, y: pos.y });
            send({ type: 'mouse', action: 'up', button: drag.button, x: pos.x, y: pos.y });
          }
        }
      }
      return;
    }

    // Untracked pointer IDs (e.g. pointerup bubbling from MobileDock / SessionHandle / SoftKeyboardBar) are ignored
    if (!touchGestureRef.current.pointers.has(e.pointerId)) return;

    const now = Date.now();
    lastTouchAtRef.current = now;
    const kind: 'up' | 'cancel' = e.type === 'pointerup' ? 'up' : 'cancel';

    const next = gesturePointerUpOrCancel(
      touchGestureRef.current,
      e.pointerId,
      local.x,
      local.y,
      kind,
      { now },
    );
    touchGestureRef.current = next.state;
    syncGestureDebug(next.state);
    runTouchEffects(next.effects);
  };

  const handleLostPointerCapture = (e: React.PointerEvent<HTMLElement>) => {
    if (e.pointerType === 'touch') {
      if (!touchGestureRef.current.pointers.has(e.pointerId)) return;
      const local = stageLocalPoint(e.clientX, e.clientY);
      const next = gesturePointerUpOrCancel(
        touchGestureRef.current,
        e.pointerId,
        local.x,
        local.y,
        'cancel',
        { now: Date.now() },
      );
      touchGestureRef.current = next.state;
      syncGestureDebug(next.state);
      runTouchEffects(next.effects);
      return;
    }

    const drag = desktopDragRef.current;
    if (drag && drag.pointerId === e.pointerId) {
      desktopDragRef.current = null;
      if (drag.mode === 'remote' && !displayPrefsRef.current.viewOnly) {
        cancelQueuedMove();
        const pos = remoteFromLocalClamped(drag.lastX, drag.lastY);
        if (pos) {
          send({ type: 'mouse', action: 'up', button: drag.button, x: pos.x, y: pos.y });
        }
      }
    }
  };

  // Step-2 pinch-zoom isolation on the remote stage:
  // Non-passive touch, iOS gesturestart/gesturechange/gestureend, and Ctrl+wheel handlers
  // keep browser-level page zoom from hijacking remote desktop gestures.
  const wheelHandlerRef = useRef<(e: WheelEvent) => void>(() => {});
  wheelHandlerRef.current = (e: WheelEvent) => {
    if (!remoteInput) return;
    e.preventDefault();
    const local = stageLocalPoint(e.clientX, e.clientY);

    if (e.ctrlKey || e.altKey || e.metaKey) {
      const factor = e.deltaY < 0 ? 1.1 : 1 / 1.1;
      setScaleAround(viewRef.current.scale * factor, local.x, local.y);
      return;
    }

    if (displayPrefsRef.current.viewOnly) return;

    const pos = remoteFromLocal(local.x, local.y);
    if (pos) {
      cancelQueuedMove();
      send({ type: 'mouse', action: 'move', x: pos.x, y: pos.y });
    }

    const { tx, ty, nextAccumX, nextAccumY } = accumulateWheelTicks(
      e.deltaX,
      e.deltaY,
      e.deltaMode,
      wheelAccumRef.current.x,
      wheelAccumRef.current.y,
      displayPrefsRef.current.reverseMouseWheel,
    );
    wheelAccumRef.current = { x: nextAccumX, y: nextAccumY };

    if (tx !== 0 || ty !== 0) {
      send({ type: 'mouse', action: 'wheel', x: tx, y: ty });
    }
  };

  useEffect(() => {
    const el = stageRef.current;
    if (!el) return;

    const isStageSurface = (target: EventTarget | null): boolean =>
      target === el || target === canvasRef.current;

    const isInsideScrollableChrome = (target: EventTarget | null): boolean => {
      if (!(target instanceof Element)) return false;
      return Boolean(
        target.closest('[data-testid="connection-panel"]') ||
          target.closest('[data-testid="session-menu"]') ||
          target.closest('[data-testid="softbar"]') ||
          target.closest('[data-testid="diagnostic-overlay"]'),
      );
    };

    const onGesture = (e: Event) => {
      if (remoteInputRef.current || isStageSurface(e.target)) {
        e.preventDefault();
      }
    };

    const touchPairMetrics = (touches: TouchList) => {
      const { x: x0, y: y0 } = stageLocalPoint(touches[0].clientX, touches[0].clientY);
      const { x: x1, y: y1 } = stageLocalPoint(touches[1].clientX, touches[1].clientY);
      return {
        midX: (x0 + x1) / 2,
        midY: (y0 + y1) / 2,
        dist: Math.hypot(x1 - x0, y1 - y0),
      };
    };

    const shouldUseTouchFallback = (touches: TouchList) =>
      touches.length === 2 && touchGestureRef.current.pointers.size < 2;

    const onTouchStart = (e: TouchEvent) => {
      const now = Date.now();
      nativeTouchInfoRef.current = { count: e.touches.length, updatedAt: now };
      if (zeroTouchTimerRef.current !== null) {
        window.clearTimeout(zeroTouchTimerRef.current);
        zeroTouchTimerRef.current = null;
      }

      if (shouldPreventStageTouch(remoteInputRef.current, isStageSurface(e.target))) {
        e.preventDefault();
        touchFallbackPinchRef.current = shouldUseTouchFallback(e.touches)
          ? touchPairMetrics(e.touches)
          : null;
      }
    };

    const onTouchMove = (e: TouchEvent) => {
      const now = Date.now();
      nativeTouchInfoRef.current = { count: e.touches.length, updatedAt: now };
      if (shouldPreventStageTouch(remoteInputRef.current, isStageSurface(e.target))) {
        e.preventDefault();
        if (!shouldUseTouchFallback(e.touches)) {
          touchFallbackPinchRef.current = null;
          return;
        }

        const cur = touchPairMetrics(e.touches);
        const prev = touchFallbackPinchRef.current;
        if (touchGestureRef.current.pointers.size === 1) {
          // A partial pointer sequence must not also become a tap or long press.
          resetAllGesturesRef.current();
        }
        touchFallbackPinchRef.current = cur;
        if (prev) {
          applyPinchStepRef.current(prev.midX, prev.midY, prev.dist, cur.midX, cur.midY, cur.dist);
        }
      }
    };

    const onTouchEndOrCancel = (e: TouchEvent) => {
      const now = Date.now();
      nativeTouchInfoRef.current = { count: e.touches.length, updatedAt: now };
      if (e.touches.length < 2) {
        touchFallbackPinchRef.current = null;
      }
      // Authoritative zombie-pointer recovery: if OS reports 0 fingers on screen while
      // tracked pointers remain (missed pointerup/pointercancel), sweep back to 'idle' safely.
      if (e.touches.length === 0 && touchGestureRef.current.pointers.size > 0) {
        if (zeroTouchTimerRef.current !== null) {
          window.clearTimeout(zeroTouchTimerRef.current);
        }
        const delay = e.type === 'touchcancel' ? 0 : 60;
        zeroTouchTimerRef.current = window.setTimeout(() => {
          zeroTouchTimerRef.current = null;
          if (
            nativeTouchInfoRef.current.count === 0 &&
            touchGestureRef.current.pointers.size > 0
          ) {
            resetAllGesturesRef.current();
          }
        }, delay);
      }
    };

    const onWindowBlur = () => {
      resetAllGesturesRef.current();
    };

    const onVisibilityChange = () => {
      if (document.hidden) {
        resetAllGesturesRef.current();
      }
    };

    const onWheelNative = (e: WheelEvent) => {
      if (remoteInputRef.current && !isInsideScrollableChrome(e.target)) {
        wheelHandlerRef.current(e);
      }
    };

    const onWindowWheel = (e: WheelEvent) => {
      if (shouldPreventWindowWheel(remoteInputRef.current, isInsideScrollableChrome(e.target))) {
        e.preventDefault();
      }
    };

    const onWindowAuxOrNavMouse = (e: MouseEvent) => {
      if (
        remoteInputRef.current &&
        isBrowserNavOrAuxButton(e.button) &&
        !isInsideScrollableChrome(e.target)
      ) {
        e.preventDefault();
      }
    };

    const onWindowDragOrSelect = (e: Event) => {
      if (remoteInputRef.current && !isInsideScrollableChrome(e.target)) {
        e.preventDefault();
      }
    };

    el.addEventListener('gesturestart', onGesture, { passive: false });
    el.addEventListener('gesturechange', onGesture, { passive: false });
    el.addEventListener('gestureend', onGesture, { passive: false });
    el.addEventListener('touchstart', onTouchStart, { passive: false });
    el.addEventListener('touchmove', onTouchMove, { passive: false });
    el.addEventListener('touchend', onTouchEndOrCancel, { passive: true });
    el.addEventListener('touchcancel', onTouchEndOrCancel, { passive: true });
    el.addEventListener('wheel', onWheelNative, { passive: false });
    window.addEventListener('blur', onWindowBlur);
    document.addEventListener('visibilitychange', onVisibilityChange);
    window.addEventListener('wheel', onWindowWheel, { passive: false, capture: true });
    window.addEventListener('mousedown', onWindowAuxOrNavMouse, true);
    window.addEventListener('auxclick', onWindowAuxOrNavMouse, true);
    window.addEventListener('dragstart', onWindowDragOrSelect, true);
    window.addEventListener('selectstart', onWindowDragOrSelect, true);

    return () => {
      if (zeroTouchTimerRef.current !== null) {
        window.clearTimeout(zeroTouchTimerRef.current);
        zeroTouchTimerRef.current = null;
      }
      el.removeEventListener('gesturestart', onGesture);
      el.removeEventListener('gesturechange', onGesture);
      el.removeEventListener('gestureend', onGesture);
      el.removeEventListener('touchstart', onTouchStart);
      el.removeEventListener('touchmove', onTouchMove);
      el.removeEventListener('touchend', onTouchEndOrCancel);
      el.removeEventListener('touchcancel', onTouchEndOrCancel);
      el.removeEventListener('wheel', onWheelNative);
      window.removeEventListener('blur', onWindowBlur);
      document.removeEventListener('visibilitychange', onVisibilityChange);
      window.removeEventListener('wheel', onWindowWheel, { capture: true });
      window.removeEventListener('mousedown', onWindowAuxOrNavMouse, true);
      window.removeEventListener('auxclick', onWindowAuxOrNavMouse, true);
      window.removeEventListener('dragstart', onWindowDragOrSelect, true);
      window.removeEventListener('selectstart', onWindowDragOrSelect, true);
    };
  }, []);

  const isMobileViewport =
    box.w > 0 ? box.w <= 768 : typeof window !== 'undefined' && window.innerWidth <= 768;

  const handleToggleTouchMode = () => {
    setRightArmed(false);
    setTouchMode((prev) => {
      const next = !prev;
      if (!next) {
        setVirtualCursorVisible(
          isCoarseDevice || isMobileViewport || Date.now() - lastTouchAtRef.current < 60_000,
        );
      } else {
        setVirtualCursorVisible(false);
      }
      return next;
    });
  };

  const handleMobileRightClick = () => {
    if (!touchMode) {
      const pt = cursorPoint();
      clickButton('right', pt.x, pt.y);
      return;
    }
    setRightArmed((r) => !r);
  };

  const isTouchLayout = isCoarseDevice || isMobileViewport;
  const showConnectSheet = phase !== 'live' || connectDialog;
  const showSessionMenu = menuOpen && phase === 'live' && !connectDialog;
  const showHandle = phase === 'live' && !connectDialog && !menuOpen && !isTouchLayout;
  const showMobileDock = phase === 'live' && !connectDialog && !softKeys && isTouchLayout;
  const liveHostTitle =
    peer?.hostname || peer?.username || (mode === 'id' ? peerId : host) || 'Remote';
  const showRemoteCursor =
    displayPrefs.showRemoteCursor &&
    !cursorEmbedded &&
    (isTouchLayout || virtualCursorVisible) &&
    phase === 'live';

  return (
    <section
      ref={stageRef}
      className={`stage${hand ? ' hand' : ''}`}
      data-testid="stage"
      data-remote-active={remoteInput ? 'true' : 'false'}
      data-scroll-style={displayPrefs.scrollStyle}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUpOrCancel}
      onPointerCancel={handlePointerUpOrCancel}
      onLostPointerCapture={handleLostPointerCapture}
      onContextMenu={(e) => {
        if (remoteInput || e.target === stageRef.current || e.target === canvasRef.current) {
          e.preventDefault();
        }
      }}
    >
      <canvas
        ref={canvasRef}
        data-testid="canvas"
        width={remoteW}
        height={remoteH}
        style={{
          left: `${view.panX}px`,
          top: `${view.panY}px`,
          width: `${remoteW * view.scale}px`,
          height: `${remoteH * view.scale}px`,
        }}
      />

      <div
        ref={cursorElRef}
        className="remote-cursor"
        style={displayPrefs.enlargeCursor ? { transform: 'scale(2)', transformOrigin: '0 0' } : undefined}
        data-testid="remote-cursor"
        hidden={!showRemoteCursor}
        aria-hidden="true"
      >
        {cursorShape ? (
          <canvas
            ref={cursorCanvasRef}
            width={cursorShape.pixels.width}
            height={cursorShape.pixels.height}
            style={{ left: -cursorShape.hotX, top: -cursorShape.hotY }}
          />
        ) : (
          <MousePointer2 size={18} fill="#fff" stroke="#171717" />
        )}
      </div>

      <input
        ref={captureRef}
        type="text"
        className="capture"
        data-testid="capture"
        aria-hidden="true"
        tabIndex={-1}
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        spellCheck={false}
        onCompositionStart={() => {
          captureComposingRef.current = true;
        }}
        onCompositionEnd={(e) => {
          captureComposingRef.current = false;
          lastCompositionEndAtRef.current = performance.now();
          const committed = e.data || e.currentTarget.value;
          e.currentTarget.value = '';
          if (committed && !displayPrefsRef.current.viewOnly) {
            sendText(committed);
          }
        }}
        onInput={(e) => {
          if (captureComposingRef.current) return;
          const val = e.currentTarget.value;
          if (val) {
            e.currentTarget.value = '';
            if (!displayPrefsRef.current.viewOnly) {
              sendText(val);
            }
          }
        }}
      />

      {pageZoomed && (
        <div
          className="page-zoom-banner"
          data-testid="page-zoom-banner"
          role="status"
          onPointerDown={(e) => e.stopPropagation()}
        >
          <span>{t(locale, 'pageZoomedBanner')}</span>
          <button
            type="button"
            className="rd-btn primary"
            style={{ minHeight: 32, padding: '4px 10px' }}
            data-testid="page-zoom-reset"
            onClick={resetPageZoom}
          >
            {t(locale, 'reset')}
          </button>
        </div>
      )}

      {showHandle && (
        <SessionHandle
          locale={locale}
          handleX={handleX}
          stageWidth={box.w}
          expanded={toolbarExpanded}
          pinned={toolbarPinned}
          hostTitle={liveHostTitle}
          displays={peer?.displays ?? []}
          activeDisplay={display}
          viewMode={viewMode}
          hasWarning={Boolean(decoderError)}
          onMoveHandle={setHandleX}
          onSetExpanded={setToolbarExpanded}
          onTogglePinned={() => setToolbarPinned((prev) => !prev)}
          onSwitchDisplay={(idx) => {
            setDisplay(idx);
            send({ type: 'switchDisplay', display: idx });
          }}
          onToggleViewMode={() => {
            if (viewMode === 'fit') {
              setViewMode('original');
              setPan({
                panX: (box.w - remoteW) / 2,
                panY: (box.h - remoteH) / 2,
              });
            } else {
              setViewMode('fit');
              setZoom(1);
              setPan({ panX: 0, panY: 0 });
            }
          }}
          onRefresh={() => send({ type: 'refresh' })}
          onToggleKeyboard={toggleSoftKeyboard}
          onCtrlAltDel={() => handleQuickKey('Ctrl+Alt+Del')}
          onOpenMenu={() => setMenuOpen(true)}
          onDisconnect={disconnect}
        />
      )}

      {qualityMonitor && !diagnosticsOpen && phase === 'live' && (
        <div className="quality-monitor" data-testid="quality-monitor">
          {routeLabel} · {t(locale, 'browserGateway')} {diagSummary.currentRttMs === null ? '—' : Math.round(diagSummary.currentRttMs)} ms · {stats.fps} FPS · {formatBitrate(stats.bitrate)}
        </div>
      )}
      {restartState && <div className="restart-notice" role="status">
        {t(locale, restartState === 'requested' ? 'restartWaiting' : 'retryWaiting')}
        <button type="button" onClick={restartState === 'requested' ? () => setRestartState(null) : disconnect}>{t(locale,'cancel')}</button>
      </div>}
      {diagnosticsOpen && phase === 'live' && (
        <DiagnosticOverlay
          locale={locale}
          stats={stats}
          summary={diagSummary}
          routeLabel={routeLabel}
          encryptionLabel={t(locale, peer?.encrypted ? 'linkEncrypted' : 'linkPlain')}
          monitor={qualityMonitor}
          onToggleMonitor={() => setQualityMonitor(value => !value)}
          hostRttMs={rtt}
          onReset={resetDiagnostics}
          onClose={() => setDiagnosticsOpen(false)}
        />
      )}

      {showMobileDock && (
        <MobileDock
          locale={locale}
          activePanel={menuOpen ? mobilePanel : null}
          onToggleKeyboard={toggleSoftKeyboard}
          onCollapse={closeChrome}
          onOpenMenu={panel => {
            setMobilePanel(menuOpen && panel === mobilePanel ? 'toolbar' : panel);
            setMenuOpen(true);
          }}
        />
      )}

      {showConnectSheet && (
        <RustDeskShell
          locale={locale}
          localePreference={localePreference}
          isDesktop={!isMobileViewport}
          theme={theme}
          displayPrefs={displayPrefs}
          onChangeLocale={changeLocale}
          onChangeTheme={changeTheme}
          onUpdateDisplayPrefs={handleUpdateDisplayPrefs}
          mode={mode}
          directIpEnabled={directIpEnabled}
          phase={phase}
          host={host}
          port={port}
          password={password}
          peerId={peerId}
          idServer={idServer}
          relayServer={relayServer}
          serverKey={serverKey}
          forceRelay={forceRelay}
          gatewayToken={gatewayToken}
          authRequired={authRequired}
          detail={detail}
          decoderError={decoderError}
          touchMode={touchMode}
          diagnosticsOpen={diagnosticsOpen}
          devices={devices}
          savedPasswords={savedPasswords}
          rememberPassword={rememberPassword}
          onChangeMode={setMode}
          onToggleDirectIp={handleToggleDirectIp}
          onChangeField={(field, val) => {
            if (field === 'host') setHost(val);
            else if (field === 'port') setPort(val);
            else if (field === 'password') setPassword(val);
            else if (field === 'peerId') setPeerId(val);
            else if (field === 'idServer') setIdServer(val);
            else if (field === 'relayServer') setRelayServer(val);
            else if (field === 'serverKey') setServerKey(val);
            else if (field === 'gatewayToken') setGatewayToken(val);
          }}
          onChangeForceRelay={setForceRelay}
          onChangeRememberPassword={handleRememberPasswordChange}
          onClearSavedPassword={handleClearSavedPassword}
          onSelectDevice={handleSelectDevice}
          onToggleFavorite={handleToggleFavorite}
          onRemoveDevice={handleRemoveDevice}
          onToggleTouchMode={handleToggleTouchMode}
          onToggleDiagnostics={() => setDiagnosticsOpen((prev) => !prev)}
          onConnect={(req) => void connectSession(req)}
          onDisconnect={disconnect}
          onDismissError={handleDismissError}
          onClose={closeChrome}
        />
      )}

      {showSessionMenu && (
        <SessionMenu
          onRestart={requestRestart}
          mobilePanel={mobilePanel}
          locale={locale}
          audioEnabled={audioEnabled}
          audioIssue={audioIssue ? t(locale, audioIssue) : null}
          onToggleAudio={toggleAudio}
          mobileLayout={isTouchLayout}
          cursorEmbedded={cursorEmbedded}
          followWindow={displayPrefs.followWindow}
          onToggleFollowWindow={() => handleUpdateDisplayPrefs({ followWindow: !displayPrefs.followWindow })}
          enlargeCursor={displayPrefs.enlargeCursor}
          onToggleEnlargeCursor={() => handleUpdateDisplayPrefs({ enlargeCursor: !displayPrefs.enlargeCursor })}
          phase={phase}
          peer={peer}
          decoderError={decoderError}
          display={display}
          viewMode={viewMode}
          zoomPercent={zoomPercent}
          hand={hand}
          touchMode={touchMode}
          rightArmed={rightArmed}
          viewOnly={displayPrefs.viewOnly}
          autoKeyboard={displayPrefs.autoKeyboard}
          onToggleAutoKeyboard={() => handleUpdateDisplayPrefs({ autoKeyboard: !displayPrefs.autoKeyboard })}
          followRemoteCursor={displayPrefs.followRemoteCursor}
          onToggleFollowCursor={() => handleUpdateDisplayPrefs({ followRemoteCursor: !displayPrefs.followRemoteCursor })}
          sizes={resolutionOptions?.display === display ? resolutionOptions.sizes : []}
          onResolution={(width, height) => send({ type: 'resolution', display, width, height })}
          showRemoteCursor={displayPrefs.showRemoteCursor}
          diagnosticsOpen={diagnosticsOpen}
          stats={stats}
          rtt={rtt}
          quality={quality}
          targetFps={targetFps}
          preferredCodec={displayPrefs.codec}
          remoteClipboard={remoteClipboard}
          onClose={closeChrome}
          onOpenConnection={() => {
            setMenuOpen(false);
            setConnectDialog(true);
          }}
          onDisconnect={disconnect}
          onToggleDiagnostics={() => {
            setDiagnosticsOpen(true);
            setMenuOpen(false);
          }}
          onToggleViewOnly={() =>
            handleUpdateDisplayPrefs({ viewOnly: !displayPrefs.viewOnly })
          }
          onToggleRemoteCursor={() =>
            handleUpdateDisplayPrefs({ showRemoteCursor: !displayPrefs.showRemoteCursor })
          }
          onFit={() => {
            setViewMode('fit');
            setZoom(1);
            setPan({ panX: 0, panY: 0 });
          }}
          onOriginal={() => {
            setViewMode('original');
            setPan({
              panX: (box.w - remoteW) / 2,
              panY: (box.h - remoteH) / 2,
            });
          }}
          onZoomOut={() => setScaleAround(viewRef.current.scale / 1.25, box.w / 2, box.h / 2)}
          onZoomIn={() => setScaleAround(viewRef.current.scale * 1.25, box.w / 2, box.h / 2)}
          onToggleHand={() => setHand((h) => !h)}
          onToggleTouchMode={handleToggleTouchMode}
          onSwitchDisplay={(idx) => {
            setDisplay(idx);
            send({ type: 'switchDisplay', display: idx });
          }}
          onRefresh={() => send({ type: 'refresh' })}
          onQuickKey={handleQuickKey}
          onRightClick={() => {
            handleMobileRightClick();
            setMenuOpen(false);
          }}
          onChangeQuality={(q, fps, nextCodec) => {
            setQuality(q);
            setTargetFps(fps);
            const activeCodec = nextCodec ?? displayPrefsRef.current.codec;
            if (nextCodec && nextCodec !== displayPrefsRef.current.codec) {
              setDisplayPrefs((prev) => {
                const updated = { ...prev, codec: nextCodec };
                persistDisplayPrefs(updated);
                return updated;
              });
            }
            send({ type: 'quality', quality: q, fps, codec: activeCodec });
          }}
          onSendClipboard={(text) => {
            if (!displayPrefsRef.current.disableClipboard) {
              send({ type: 'clipboard', text });
            }
          }}
        />
      )}

      {softKeys && remoteInput && (
        <SoftKeyboardBar
          locale={locale}
          inputRef={softKeyboardInputRef}
          onSendText={sendText}
          onSendControl={(action, control) => send({ type: 'key', action, control })}
          onSendChord={sendChord}
          onHide={hideSoftKeyboard}
        />
      )}
    </section>
  );
}
