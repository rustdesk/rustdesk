import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  ConnectionSheet,
  type ConnectionMode,
  type ConnectionPhase,
} from './components/ConnectionSheet';
import { DiagnosticOverlay } from './components/DiagnosticOverlay';
import { MobileDock } from './components/MobileDock';
import { SessionHandle } from './components/SessionHandle';
import { SessionMenu, type SessionStats } from './components/SessionMenu';
import { SoftKeyboardBar } from './components/SoftKeyboardBar';
import { probeCodecs, RemoteDecoder, webCodecsStatus } from './lib/decoder';
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
  consumeThreeFingerScroll,
  fingerDown,
  fingerUp,
  isPageZoomed,
  isTwoFingerTap,
  keyboardInset,
  LONG_PRESS_MS,
  shouldCancelLongPress,
  TAP_SLOP_PX,
} from './lib/gestures';
import {
  LOCALE_STORAGE_KEY,
  nextLocale,
  resolveInitialLocale,
  t,
  type I18nKey,
  type Locale,
} from './lib/i18n';
import { isModifier, mapPhysicalKeyDown, mapPhysicalKeyUp } from './lib/keys';
import {
  clampPan,
  computeNextScaleState,
  computeViewTransform,
  fitScale,
  mapFromRemote,
  mapToRemote,
  type ViewMode,
  type ViewTransform,
} from './lib/mapping';
import {
  decodeFrame,
  wsUrl,
  type ClientMessage,
  type PeerMessage,
  type QualityLevel,
  type ServerMessage,
} from './lib/protocol';

const EMPTY_STATS: SessionStats = {
  codec: '—',
  width: 0,
  height: 0,
  fps: 0,
  delayMs: 0,
  bitrate: 0,
  decoded: 0,
};

function detectCoarsePointer(): boolean {
  if (typeof window === 'undefined') return false;
  return (
    window.matchMedia?.('(pointer: coarse)').matches ||
    (typeof navigator !== 'undefined' && navigator.maxTouchPoints > 0)
  );
}

interface PinchState {
  originX: number;
  originY: number;
  originDist: number;
  prevX: number;
  prevY: number;
  prevDist: number;
}

const PREFS_STORAGE_KEY = 'rd_web_prefs';

interface SavedConnectionPrefs {
  mode?: ConnectionMode;
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

  // Connection form & session state (non-secret fields persisted locally; passwords never stored)
  const [mode, setMode] = useState<ConnectionMode>(
    prefs.mode === 'direct' || prefs.mode === 'id' || prefs.mode === 'demo' ? prefs.mode : 'direct',
  );
  const [host, setHost] = useState(typeof prefs.host === 'string' ? prefs.host : '127.0.0.1');
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

  useEffect(() => {
    try {
      const next: SavedConnectionPrefs = {
        mode,
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
  }, [mode, host, port, peerId, idServer, relayServer, serverKey, forceRelay]);

  // Bilingual UI locale (EN / 中文)
  const [locale, setLocale] = useState<Locale>(() => {
    try {
      return resolveInitialLocale(window.localStorage.getItem(LOCALE_STORAGE_KEY));
    } catch {
      return 'en';
    }
  });

  const toggleLocale = useCallback(() => {
    setLocale((prev) => {
      const next = nextLocale(prev);
      try {
        window.localStorage.setItem(LOCALE_STORAGE_KEY, next);
      } catch {
        // Ignore storage quota or private-mode errors
      }
      return next;
    });
  }, []);

  useEffect(() => {
    document.documentElement.lang = locale === 'zh' ? 'zh-CN' : 'en';
  }, [locale]);

  const [phase, setPhase] = useState<ConnectionPhase>('idle');
  const [detailKey, setDetailKey] = useState<I18nKey | null>('detailIdle');
  const [rawDetail, setRawDetail] = useState('');
  const detail = detailKey ? t(locale, detailKey) : rawDetail;
  const setStatusDetail = useCallback((key: I18nKey | null, customText = '') => {
    setDetailKey(key);
    setRawDetail(customText);
  }, []);
  const [peer, setPeer] = useState<PeerMessage | null>(null);
  const [display, setDisplay] = useState(0);
  const [stats, setStats] = useState<SessionStats>(EMPTY_STATS);
  const [decoderError, setDecoderError] = useState(initialWebCodecs.ok ? '' : initialWebCodecs.reason);
  const [rtt, setRtt] = useState(0);
  const [quality, setQuality] = useState<QualityLevel>('balanced');
  const [targetFps, setTargetFps] = useState(30);
  const [remoteClipboard, setRemoteClipboard] = useState('');

  // Viewport & chrome state
  const [viewMode, setViewMode] = useState<ViewMode>('fit');
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ panX: 0, panY: 0 });
  const [hand, setHand] = useState(false);
  const [touchMode, setTouchMode] = useState<boolean>(isCoarseDevice);
  const [rightArmed, setRightArmed] = useState(false);
  const [handleX, setHandleX] = useState(0.5);
  const [box, setBox] = useState({ w: 0, h: 0 });
  const [menuOpen, setMenuOpen] = useState(false);
  const [connectDialog, setConnectDialog] = useState(false);
  const [softKeys, setSoftKeys] = useState(false);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [virtualCursorVisible, setVirtualCursorVisible] = useState(false);
  const [pageZoomed, setPageZoomed] = useState(false);
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
  const captureRef = useRef<HTMLInputElement>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const decoderRef = useRef<RemoteDecoder | null>(null);
  const sizeRef = useRef({ width: 1280, height: 720 });
  const phaseRef = useRef<ConnectionPhase>(phase);
  phaseRef.current = phase;

  const chromeRef = useRef({ menu: false, dialog: true });
  chromeRef.current = { menu: menuOpen, dialog: connectDialog || phase !== 'live' };

  // Stats accumulation ref (settles every 500ms)
  const statAccRef = useRef({ lastTs: 0, frames: 0, bytes: 0, decoded: 0 });

  // Mouse & touch gesture refs
  const cursorRef = useRef({ x: 640, y: 360 });
  const pointersRef = useRef<Map<number, { x: number; y: number }>>(new Map());
  const pinchRef = useRef<PinchState | null>(null);
  const threeFingerYRef = useRef<number | null>(null);
  const threeFingerAccumRef = useRef(0);
  const touchTapRef = useRef<{
    x: number;
    y: number;
    t: number;
    dragging: boolean;
    longFired: boolean;
  }>({ x: 0, y: 0, t: 0, dragging: false, longFired: false });
  const lastTapUpRef = useRef<number | null>(null);
  const suppressTapRef = useRef(false);
  const lastTouchAtRef = useRef(0);
  const longTimerRef = useRef<number | null>(null);
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
  );
  const viewRef = useRef(view);
  viewRef.current = view;

  const remoteInput = phase === 'live' && !menuOpen && !connectDialog;
  const fit = fitScale(box.w, box.h, remoteW, remoteH);
  const zoomPercent = fit > 0 ? Math.round((view.scale / fit) * 100) : 100;

  // Send JSON helper
  const send = useCallback((msg: ClientMessage) => {
    const sock = wsRef.current;
    if (sock && sock.readyState === WebSocket.OPEN) {
      sock.send(JSON.stringify(msg));
    }
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

  const clickButton = useCallback(
    (button: 'left' | 'right' | 'middle', x: number, y: number) => {
      finishPendingClick();
      send({ type: 'mouse', action: 'move', x, y });
      send({ type: 'mouse', action: 'down', button, x, y });
      const timer = window.setTimeout(() => {
        pendingClickRef.current = null;
        send({ type: 'mouse', action: 'up', button, x, y });
      }, 40);
      pendingClickRef.current = { button, x, y, timer };
    },
    [finishPendingClick, send],
  );

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
    if (window.__rd) {
      window.__rd.cursorX = Math.round(cx);
      window.__rd.cursorY = Math.round(cy);
    }
    const el = cursorElRef.current;
    if (el) {
      const screen = mapFromRemote(cx, cy, viewRef.current);
      el.style.left = `${screen.x}px`;
      el.style.top = `${screen.y}px`;
    }
  }, []);

  // Keep virtual cursor DOM position synchronized on view changes
  useEffect(() => {
    const el = cursorElRef.current;
    if (!el) return;
    const screen = mapFromRemote(cursorRef.current.x, cursorRef.current.y, view);
    el.style.left = `${screen.x}px`;
    el.style.top = `${screen.y}px`;
  }, [view.scale, view.panX, view.panY, virtualCursorVisible]);

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
    if (!diagnosticsOpen || (phase !== 'live' && phase !== 'connecting')) {
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
  }, [diagnosticsOpen, phase]);

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
      const rect = el.getBoundingClientRect();
      const w = Math.round(rect.width);
      const h = Math.round(rect.height);
      setBox({ w, h });
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
    const syncViewport = () => {
      const inset = Math.round(keyboardInset(window.innerHeight, vv.height, vv.offsetTop));
      document.documentElement.style.setProperty('--kb', `${inset}px`);
      const zoomed = isPageZoomed(vv.scale);
      setPageZoomed(zoomed);
      if (zoomed && !prevZoomed) {
        // Attempt automatic viewport scale restoration first; keep banner if browser retains zoom
        resetPageZoom();
      }
      prevZoomed = zoomed;
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

  // Track fullscreen changes
  useEffect(() => {
    const onFs = () => {
      setIsFullscreen(Boolean(document.fullscreenElement));
    };
    document.addEventListener('fullscreenchange', onFs);
    return () => document.removeEventListener('fullscreenchange', onFs);
  }, []);

  const closeChrome = useCallback(() => {
    chromeRef.current = { menu: false, dialog: false };
    setMenuOpen(false);
    setConnectDialog(false);
    captureRef.current?.focus();
  }, []);

  const ensureDecoder = useCallback(() => {
    if (!decoderRef.current) {
      decoderRef.current = new RemoteDecoder({
        onOutput: (w, h, decodedCount) => {
          sizeRef.current = { width: w, height: h };
          statAccRef.current.decoded = decodedCount;
          setStats((prev) =>
            prev.width === w && prev.height === h && prev.decoded === decodedCount
              ? prev
              : { ...prev, width: w, height: h, decoded: decodedCount },
          );
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
  }, [send]);

  const noteFrame = useCallback(
    (codecName: string, w: number, h: number, byteLen: number, delayMs: number) => {
      const now = performance.now();
      const acc = statAccRef.current;
      acc.frames += 1;
      acc.bytes += byteLen;
      if (acc.lastTs === 0) {
        acc.lastTs = now;
        setStats((prev) => ({
          ...prev,
          codec: codecName,
          width: w > 0 ? w : prev.width,
          height: h > 0 ? h : prev.height,
          decoded: acc.decoded,
        }));
        return;
      }
      const elapsed = now - acc.lastTs;
      if (elapsed >= 500) {
        const fps = Math.round((acc.frames * 1000) / elapsed);
        const bitrate = Math.round((acc.bytes * 8 * 1000) / elapsed);
        acc.frames = 0;
        acc.bytes = 0;
        acc.lastTs = now;
        setStats((prev) => ({
          ...prev,
          codec: codecName,
          width: w > 0 ? w : prev.width,
          height: h > 0 ? h : prev.height,
          fps,
          bitrate,
          delayMs: Math.max(0, Math.round(delayMs)),
          decoded: acc.decoded,
        }));
      }
    },
    [],
  );

  const connectSession = useCallback(
    async (overrideMode?: ConnectionMode) => {
      const activeMode = overrideMode ?? mode;
      const wc = webCodecsStatus();
      if (!wc.ok) {
        setPhase('error');
        setDecoderError(wc.reason);
        setStatusDetail(null, wc.reason);
        setConnectDialog(true);
        return;
      }

      if (activeMode === 'direct' && !host.trim()) {
        setPhase('error');
        setStatusDetail('detailMissingHost');
        return;
      }
      if (activeMode === 'id' && (!peerId.replace(/\s+/g, '') || !idServer.trim())) {
        setPhase('error');
        setStatusDetail('detailMissingId');
        return;
      }

      const support = await probeCodecs();
      if (!support.vp8 && !support.vp9 && !support.h264 && !support.h265 && !support.av1) {
        setPhase('error');
        setDecoderError(t(locale, 'detailNoCodecs'));
        setStatusDetail('detailNoCodecs');
        setConnectDialog(true);
        return;
      }

      // Close previous socket & decoder cleanly
      if (wsRef.current) {
        wsRef.current.onclose = null;
        wsRef.current.onerror = null;
        wsRef.current.onmessage = null;
        wsRef.current.close();
        wsRef.current = null;
      }
      decoderRef.current?.close();
      statAccRef.current = { lastTs: 0, frames: 0, bytes: 0, decoded: 0 };

      setPeer(null);
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

      sock.onopen = () => {
        if (wsRef.current !== sock) return;
        setStatusDetail('detailAuthenticating');
        if (activeMode === 'demo') {
          sock.send(
            JSON.stringify({
              type: 'connect',
              mode: 'demo',
              token: gatewayToken,
              codecs: support,
            }),
          );
        } else if (activeMode === 'id') {
          sock.send(
            JSON.stringify({
              type: 'connect',
              mode: 'id',
              peerId: peerId.replace(/\s+/g, ''),
              idServer: idServer.trim(),
              relayServer: relayServer.trim(),
              key: serverKey.trim(),
              forceRelay,
              password,
              token: gatewayToken,
              codecs: support,
            }),
          );
        } else {
          sock.send(
            JSON.stringify({
              type: 'connect',
              mode: 'direct',
              host: host.trim(),
              port: Number(port) || 21118,
              password,
              token: gatewayToken,
              codecs: support,
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
              setPhase('live');
              setConnectDialog(false);
              setMenuOpen(false);
              setStatusDetail(
                msg.detail === 'encrypted' ? 'detailEncryptedSession' : 'detailDirectSession',
              );
              captureRef.current?.focus();
            } else if (msg.state === 'connecting') {
              setPhase('connecting');
              if (msg.detail) {
                setStatusDetail(null, msg.detail);
              } else {
                setStatusDetail('connecting');
              }
            } else if (msg.state === 'error') {
              setPhase('error');
              setConnectDialog(true);
              setMenuOpen(false);
              if (msg.detail && /token/i.test(msg.detail)) {
                setAuthRequired(true);
                setStatusDetail('detailTokenError');
              } else if (msg.detail) {
                setStatusDetail(null, msg.detail);
              } else {
                setStatusDetail('detailHostRejected');
              }
            } else if (msg.state === 'closed') {
              setPhase('idle');
              setStatusDetail('detailDisconnected');
            }
            break;

          case 'peer': {
            setPeer(msg);
            setDisplay(msg.currentDisplay);
            const chosen = msg.displays[msg.currentDisplay] ?? msg.displays[0];
            if (chosen && chosen.width > 0 && chosen.height > 0) {
              sizeRef.current = { width: chosen.width, height: chosen.height };
              setStats((prev) => ({ ...prev, width: chosen.width, height: chosen.height }));
              placeCursor(chosen.width / 2, chosen.height / 2);
            }
            break;
          }

          case 'display':
            setDisplay(msg.display);
            if (msg.width > 0 && msg.height > 0) {
              sizeRef.current = { width: msg.width, height: msg.height };
              setStats((prev) => ({ ...prev, width: msg.width, height: msg.height }));
              setViewMode('fit');
              setZoom(1);
              setPan({ panX: 0, panY: 0 });
            }
            break;

          case 'clipboard':
            setRemoteClipboard(msg.text);
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
            break;

          default:
            break;
        }
      };

      sock.onerror = () => {
        if (wsRef.current !== sock) return;
        setPhase('error');
        setConnectDialog(true);
        setStatusDetail('detailGatewayError');
      };

      sock.onclose = () => {
        if (wsRef.current !== sock) return;
        wsRef.current = null;
        if (phaseRef.current !== 'error') {
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

  // Optional ?autostart=1 support for quick testing (starts Virtual Demo)
  const autoStartedRef = useRef(false);
  useEffect(() => {
    if (autoStartedRef.current) return;
    const params = new URLSearchParams(window.location.search);
    if (params.get('autostart') === '1') {
      autoStartedRef.current = true;
      setMode('demo');
      void connectSession('demo');
    }
  }, [connectSession]);

  const disconnect = useCallback(() => {
    finishPendingClick();
    clearLongPress();
    send({ type: 'disconnect' });
    if (wsRef.current) {
      wsRef.current.onclose = null;
      wsRef.current.close();
      wsRef.current = null;
    }
    decoderRef.current?.close();
    setMenuOpen(false);
    setConnectDialog(false);
    setSoftKeys(false);
    resetDiagnostics();
    setPhase('idle');
    setStatusDetail('detailDisconnected');
  }, [clearLongPress, finishPendingClick, resetDiagnostics, send, setStatusDetail]);

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

  // Scale helper
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
      );
      setViewMode(next.viewMode);
      setZoom(next.zoom);
      setPan(next.pan);
    },
    [box.w, box.h, remoteW, remoteH],
  );

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
      );
      setPan(nextPan);
      if (cur.scale > curFit + 0.01 && viewMode === 'fit') {
        setViewMode('custom');
        setZoom(curFit > 0 ? cur.scale / curFit : 1);
      }
    },
    [box.w, box.h, remoteW, remoteH, viewMode],
  );

  // Send text per Unicode codepoint
  const sendText = useCallback(
    (text: string) => {
      for (const ch of text) {
        send({ type: 'text', text: ch });
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

  // Global physical keyboard listener (capture phase + focus isolation + Appendix B #4 fix)
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

      if (phaseRef.current !== 'live' || isInsideChromeInput(e.target)) {
        return;
      }

      const mapped = mapPhysicalKeyDown(e);
      if (mapped) {
        e.preventDefault();
        send(mapped);
      }
    };

    const onKeyUp = (e: KeyboardEvent) => {
      if (chromeRef.current.menu || chromeRef.current.dialog) {
        return;
      }
      if (phaseRef.current !== 'live' || isInsideChromeInput(e.target)) {
        return;
      }
      const mapped = mapPhysicalKeyUp(e);
      if (mapped && mapped.type === 'key') {
        e.preventDefault();
        send(mapped);
      }
    };

    window.addEventListener('keydown', onKeyDown, true);
    window.addEventListener('keyup', onKeyUp, true);
    return () => {
      window.removeEventListener('keydown', onKeyDown, true);
      window.removeEventListener('keyup', onKeyUp, true);
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

  const releaseStuckDrag = useCallback(() => {
    if (!touchTapRef.current.dragging) return;
    touchTapRef.current.dragging = false;
    flushMove();
    const pos = touchMode
      ? remoteFromLocal(touchTapRef.current.x, touchTapRef.current.y)
      : cursorPoint();
    if (pos) {
      send({ type: 'mouse', action: 'up', button: 'left', x: pos.x, y: pos.y });
    }
  }, [cursorPoint, flushMove, remoteFromLocal, send, touchMode]);

  const pinchSnapshot = (): PinchState | null => {
    const pts = Array.from(pointersRef.current.values());
    if (pts.length < 2) return null;
    const midX = (pts[0].x + pts[1].x) / 2;
    const midY = (pts[0].y + pts[1].y) / 2;
    const dist = Math.hypot(pts[1].x - pts[0].x, pts[1].y - pts[0].y);
    return {
      originX: midX,
      originY: midY,
      originDist: dist,
      prevX: midX,
      prevY: midY,
      prevDist: dist,
    };
  };

  // Pointer handlers on stage
  const handlePointerDown = (e: React.PointerEvent<HTMLElement>) => {
    if (!remoteInput) return;

    const isTouch = e.pointerType === 'touch';
    // Ignore browser-synthesized mouse events for 700ms after touch
    if (!isTouch && Date.now() - lastTouchAtRef.current < 700) {
      return;
    }

    const local = stageLocalPoint(e.clientX, e.clientY);

    if (!isTouch) {
      // Desktop mouse interaction
      // Appendix B fix #2: hide virtual cursor dot during desktop fine-pointer mouse use
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

      const pos = remoteFromLocal(local.x, local.y);
      if (!pos) return;
      const btn: 'left' | 'right' | 'middle' = rightArmed
        ? 'right'
        : e.button === 2
          ? 'right'
          : e.button === 1
            ? 'middle'
            : 'left';
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
      send({ type: 'mouse', action: 'down', button: btn, x: pos.x, y: pos.y });
      return;
    }

    // Touch interaction
    e.preventDefault();
    lastTouchAtRef.current = Date.now();
    if (!touchMode) {
      setVirtualCursorVisible(true);
    }

    // Clean up stale pointers if a new primary pointer arrives unexpectedly
    if (e.isPrimary && pointersRef.current.size > 0) {
      pointersRef.current.clear();
      pinchRef.current = null;
      threeFingerYRef.current = null;
      threeFingerAccumRef.current = 0;
      clearLongPress();
      releaseStuckDrag();
    }

    pointersRef.current.set(e.pointerId, local);
    try {
      e.currentTarget.setPointerCapture(e.pointerId);
    } catch {
      // Ignore NotFoundError on synthetic or rapid multi-touch pointers
    }

    // Three-finger vertical scroll (M11)
    if (pointersRef.current.size >= 3) {
      clearLongPress();
      releaseStuckDrag();
      pinchRef.current = null;
      suppressTapRef.current = true;
      const pts = Array.from(pointersRef.current.values());
      threeFingerYRef.current = pts.reduce((acc, p) => acc + p.y, 0) / pts.length;
      threeFingerAccumRef.current = 0;
      return;
    }

    // Two-finger gesture start
    if (pointersRef.current.size === 2) {
      clearLongPress();
      releaseStuckDrag();
      pinchRef.current = pinchSnapshot();
      suppressTapRef.current = true;
      return;
    }

    if (hand) {
      desktopDragRef.current = {
        pointerId: e.pointerId,
        mode: 'pan',
        button: 'left',
        lastX: local.x,
        lastY: local.y,
      };
      return;
    }

    // Single-finger down
    const now = Date.now();
    const sinceLast = lastTapUpRef.current !== null ? now - lastTapUpRef.current : null;
    const start = fingerDown(touchMode ? 'touch' : 'mouse', sinceLast);
    touchTapRef.current = {
      x: local.x,
      y: local.y,
      t: now,
      dragging: start === 'drag-lock',
      longFired: false,
    };

    if (start === 'drag-lock') {
      finishPendingClick();
      const pt = cursorPoint();
      send({ type: 'mouse', action: 'move', x: pt.x, y: pt.y });
      send({ type: 'mouse', action: 'down', button: 'left', x: pt.x, y: pt.y });
      return;
    }

    if (!touchMode && cursorRef.current.x === 0 && cursorRef.current.y === 0) {
      placeCursor(remoteW / 2, remoteH / 2);
    }

    // Arm 500ms long press -> right click
    clearLongPress();
    longTimerRef.current = window.setTimeout(() => {
      if (pointersRef.current.size !== 1 || touchTapRef.current.dragging) return;
      touchTapRef.current.longFired = true;
      const pos = touchMode
        ? remoteFromLocal(touchTapRef.current.x, touchTapRef.current.y)
        : cursorPoint();
      if (pos) {
        clickButton('right', pos.x, pos.y);
      }
    }, LONG_PRESS_MS);
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLElement>) => {
    if (!remoteInput) return;

    const isTouch = e.pointerType === 'touch';
    if (!isTouch && Date.now() - lastTouchAtRef.current < 700) {
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

      const pos = remoteFromLocal(local.x, local.y);
      if (pos) {
        queueMove(pos);
      }
      return;
    }

    // Touch move
    if (!pointersRef.current.has(e.pointerId)) return;
    const prev = pointersRef.current.get(e.pointerId)!;
    pointersRef.current.set(e.pointerId, local);

    // Three-finger vertical scroll (M11)
    if (pointersRef.current.size >= 3 && threeFingerYRef.current !== null) {
      const pts = Array.from(pointersRef.current.values());
      const avgY = pts.reduce((acc, p) => acc + p.y, 0) / pts.length;
      const dy = avgY - threeFingerYRef.current;
      threeFingerYRef.current = avgY;
      threeFingerAccumRef.current += dy;
      const { ticks, remainder } = consumeThreeFingerScroll(threeFingerAccumRef.current);
      threeFingerAccumRef.current = remainder;
      if (ticks !== 0) {
        send({ type: 'mouse', action: 'wheel', x: 0, y: ticks > 0 ? 1 : -1 });
      }
      return;
    }

    // Two-finger pinch / pan
    if (pointersRef.current.size === 2 && pinchRef.current) {
      const pts = Array.from(pointersRef.current.values());
      const midX = (pts[0].x + pts[1].x) / 2;
      const midY = (pts[0].y + pts[1].y) / 2;
      const dist = Math.hypot(pts[1].x - pts[0].x, pts[1].y - pts[0].y);
      const scaleFactor = pinchRef.current.prevDist > 0 ? dist / pinchRef.current.prevDist : 1;

      if (Math.abs(scaleFactor - 1) > 0.02) {
        setScaleAround(viewRef.current.scale * scaleFactor, midX, midY);
      } else {
        const dx = midX - pinchRef.current.prevX;
        const dy = midY - pinchRef.current.prevY;
        panBy(dx, dy);
      }

      pinchRef.current = {
        ...pinchRef.current,
        prevX: midX,
        prevY: midY,
        prevDist: dist,
      };
      return;
    }

    if (hand && desktopDragRef.current?.pointerId === e.pointerId) {
      const dx = local.x - desktopDragRef.current.lastX;
      const dy = local.y - desktopDragRef.current.lastY;
      desktopDragRef.current.lastX = local.x;
      desktopDragRef.current.lastY = local.y;
      panBy(dx, dy);
      return;
    }

    if (suppressTapRef.current || touchTapRef.current.longFired) return;

    const movedTotal = Math.hypot(
      local.x - touchTapRef.current.x,
      local.y - touchTapRef.current.y,
    );

    if (!touchMode) {
      // Mouse mode on touch device:
      // Appendix B fix #1: cancel long press once finger moves beyond TAP_SLOP_PX!
      if (shouldCancelLongPress(movedTotal)) {
        clearLongPress();
      }
      const dx = (local.x - prev.x) / Math.max(0.01, viewRef.current.scale);
      const dy = (local.y - prev.y) / Math.max(0.01, viewRef.current.scale);
      placeCursor(cursorRef.current.x + dx, cursorRef.current.y + dy);
      // Appendix B fix #3: forward throttled hover moves too, not just during drag-lock
      queueMove(cursorPoint());
      return;
    }

    // Touch mode: drag past TAP_SLOP_PX begins remote left-drag from touchdown point
    if (!touchTapRef.current.dragging && movedTotal > TAP_SLOP_PX) {
      clearLongPress();
      finishPendingClick();
      touchTapRef.current.dragging = true;
      const startPos = remoteFromLocal(touchTapRef.current.x, touchTapRef.current.y);
      if (startPos) {
        send({ type: 'mouse', action: 'move', x: startPos.x, y: startPos.y });
        send({ type: 'mouse', action: 'down', button: 'left', x: startPos.x, y: startPos.y });
      }
    }

    if (touchTapRef.current.dragging) {
      const curPos = remoteFromLocal(local.x, local.y);
      if (curPos) {
        queueMove(curPos);
      }
    }
  };

  const handlePointerUpOrCancel = (e: React.PointerEvent<HTMLElement>) => {
    const isTouch = e.pointerType === 'touch';
    if (!isTouch && Date.now() - lastTouchAtRef.current < 700) {
      return;
    }

    const local = stageLocalPoint(e.clientX, e.clientY);

    if (!isTouch) {
      const drag = desktopDragRef.current;
      if (drag && drag.pointerId === e.pointerId) {
        desktopDragRef.current = null;
        if (drag.mode === 'remote' && remoteInput) {
          flushMove();
          const pos = remoteFromLocal(local.x, local.y);
          if (pos) {
            send({ type: 'mouse', action: 'up', button: drag.button, x: pos.x, y: pos.y });
          }
        }
      }
      return;
    }

    // Touch up / cancel
    const wasTwoFinger = pointersRef.current.size === 2 && pinchRef.current !== null;
    const savedPinch = pinchRef.current;
    pointersRef.current.delete(e.pointerId);

    if (desktopDragRef.current?.pointerId === e.pointerId) {
      desktopDragRef.current = null;
    }

    if (pointersRef.current.size < 3) {
      threeFingerYRef.current = null;
      threeFingerAccumRef.current = 0;
    }

    if (wasTwoFinger && savedPinch) {
      pinchRef.current = null;
      const midShift = Math.hypot(
        savedPinch.prevX - savedPinch.originX,
        savedPinch.prevY - savedPinch.originY,
      );
      const distDelta = savedPinch.prevDist - savedPinch.originDist;
      if (isTwoFingerTap(midShift, distDelta)) {
        // Appendix B fix #7: in mouse mode, target local cursor position for two-finger right-click
        const pos = touchMode ? remoteFromLocal(local.x, local.y) : cursorPoint();
        if (pos) {
          clickButton('right', pos.x, pos.y);
        }
      }
      suppressTapRef.current = true;
      return;
    }

    if (suppressTapRef.current) {
      releaseStuckDrag();
      if (pointersRef.current.size === 0) {
        suppressTapRef.current = false;
      }
      return;
    }

    clearLongPress();
    const moved = Math.hypot(
      local.x - touchTapRef.current.x,
      local.y - touchTapRef.current.y,
    );
    const action = fingerUp({
      moved,
      longFired: touchTapRef.current.longFired,
      dragging: touchTapRef.current.dragging,
    });
    touchTapRef.current.longFired = false;
    touchTapRef.current.dragging = false;

    if (!remoteInput) return;

    if (action === 'end-drag') {
      flushMove();
      const pos = touchMode ? remoteFromLocal(local.x, local.y) : cursorPoint();
      if (pos) {
        send({ type: 'mouse', action: 'up', button: 'left', x: pos.x, y: pos.y });
      }
    } else if (action === 'left-click') {
      lastTapUpRef.current = Date.now();
      const pos = touchMode
        ? remoteFromLocal(touchTapRef.current.x, touchTapRef.current.y)
        : cursorPoint();
      if (pos) {
        const btn = rightArmed ? 'right' : 'left';
        if (rightArmed) setRightArmed(false);
        clickButton(btn, pos.x, pos.y);
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

    const dx = Math.sign(e.deltaX);
    const dy = -Math.sign(e.deltaY);
    if (dx !== 0 || dy !== 0) {
      send({ type: 'mouse', action: 'wheel', x: dx, y: dy });
    }
  };

  useEffect(() => {
    const el = stageRef.current;
    if (!el) return;

    const isStageSurface = (target: EventTarget | null): boolean =>
      target === el || target === canvasRef.current;

    const onGesture = (e: Event) => {
      if (isStageSurface(e.target) || (e.target instanceof Node && el.contains(e.target))) {
        e.preventDefault();
      }
    };

    const onTouchStart = (e: TouchEvent) => {
      if (isStageSurface(e.target) && e.touches.length >= 2) {
        e.preventDefault();
      }
    };

    const onTouchMove = (e: TouchEvent) => {
      if (isStageSurface(e.target)) {
        e.preventDefault();
      }
    };

    const onWheelNative = (e: WheelEvent) => {
      if (isStageSurface(e.target)) {
        wheelHandlerRef.current(e);
      }
    };

    el.addEventListener('gesturestart', onGesture, { passive: false });
    el.addEventListener('gesturechange', onGesture, { passive: false });
    el.addEventListener('gestureend', onGesture, { passive: false });
    el.addEventListener('touchstart', onTouchStart, { passive: false });
    el.addEventListener('touchmove', onTouchMove, { passive: false });
    el.addEventListener('wheel', onWheelNative, { passive: false });

    return () => {
      el.removeEventListener('gesturestart', onGesture);
      el.removeEventListener('gesturechange', onGesture);
      el.removeEventListener('gestureend', onGesture);
      el.removeEventListener('touchstart', onTouchStart);
      el.removeEventListener('touchmove', onTouchMove);
      el.removeEventListener('wheel', onWheelNative);
    };
  }, []);

  const handleToggleFullscreen = () => {
    const el = stageRef.current;
    if (!el) return;
    if (document.fullscreenElement) {
      document.exitFullscreen?.().catch(() => {});
    } else {
      el.requestFullscreen?.().catch(() => {});
    }
  };

  const isMobileViewport = box.w > 0 && box.w <= 768;

  const handleToggleTouchMode = () => {
    setRightArmed(false);
    setTouchMode((prev) => {
      const next = !prev;
      if (!next) {
        placeCursor(remoteW / 2, remoteH / 2);
        // Show virtual cursor dot when switching to Mouse mode on touch or mobile viewports
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

  const showConnectSheet = phase !== 'live' || connectDialog;
  const showSessionMenu = menuOpen && phase === 'live' && !connectDialog;
  const showHandle = phase === 'live' && !connectDialog && !menuOpen;
  const showMobileDock = remoteInput && !softKeys && (isCoarseDevice || isMobileViewport);
  // Appendix B fix #2: hide virtual cursor dot on desktop fine-pointer sessions
  const showRemoteCursor = !touchMode && virtualCursorVisible && phase === 'live';

  return (
    <section
      ref={stageRef}
      className={`stage${hand ? ' hand' : ''}`}
      data-testid="stage"
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUpOrCancel}
      onPointerCancel={handlePointerUpOrCancel}
      onContextMenu={(e) => e.preventDefault()}
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
        data-testid="remote-cursor"
        hidden={!showRemoteCursor}
      />

      <input
        ref={captureRef}
        className="capture"
        data-testid="capture"
        aria-hidden="true"
        tabIndex={-1}
        readOnly
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
          hasWarning={Boolean(decoderError)}
          onMoveHandle={setHandleX}
          onOpenMenu={() => setMenuOpen(true)}
        />
      )}

      {diagnosticsOpen && phase === 'live' && (
        <DiagnosticOverlay
          locale={locale}
          summary={diagSummary}
          hostRttMs={rtt || stats.delayMs}
          onReset={resetDiagnostics}
          onClose={() => setDiagnosticsOpen(false)}
        />
      )}

      {showMobileDock && (
        <MobileDock
          locale={locale}
          touchMode={touchMode}
          rightArmed={rightArmed}
          onToggleKeyboard={() => setSoftKeys(true)}
          onToggleTouchMode={handleToggleTouchMode}
          onRightClickAction={handleMobileRightClick}
          onOpenMenu={() => setMenuOpen(true)}
        />
      )}

      {showConnectSheet && (
        <ConnectionSheet
          locale={locale}
          onToggleLocale={toggleLocale}
          mode={mode}
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
          onChangeMode={setMode}
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
          onConnect={() => void connectSession()}
          onDisconnect={disconnect}
          onClose={closeChrome}
        />
      )}

      {showSessionMenu && (
        <SessionMenu
          locale={locale}
          onToggleLocale={toggleLocale}
          phase={phase}
          peer={peer}
          detail={detail}
          decoderError={decoderError}
          display={display}
          viewMode={viewMode}
          zoomPercent={zoomPercent}
          hand={hand}
          touchMode={touchMode}
          rightArmed={rightArmed}
          softKeys={softKeys}
          isFullscreen={isFullscreen}
          diagnosticsOpen={diagnosticsOpen}
          stats={stats}
          rtt={rtt}
          quality={quality}
          targetFps={targetFps}
          remoteClipboard={remoteClipboard}
          onClose={closeChrome}
          onOpenConnection={() => {
            setMenuOpen(false);
            setConnectDialog(true);
          }}
          onDisconnect={disconnect}
          onToggleDiagnostics={() => {
            setDiagnosticsOpen((prev) => !prev);
            setMenuOpen(false);
          }}
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
          onZoomOut={() => setScaleAround(viewRef.current.scale / 1.15, box.w / 2, box.h / 2)}
          onZoomIn={() => setScaleAround(viewRef.current.scale * 1.15, box.w / 2, box.h / 2)}
          onToggleHand={() => setHand((h) => !h)}
          onToggleTouchMode={handleToggleTouchMode}
          onSwitchDisplay={(idx) => {
            setDisplay(idx);
            send({ type: 'switchDisplay', display: idx });
          }}
          onRefresh={() => send({ type: 'refresh' })}
          onToggleFullscreen={handleToggleFullscreen}
          onQuickKey={handleQuickKey}
          onToggleRightArmed={() => setRightArmed((r) => !r)}
          onToggleKeyboard={() => {
            setSoftKeys((s) => !s);
            closeChrome();
          }}
          onChangeQuality={(q, fps) => {
            setQuality(q);
            setTargetFps(fps);
            send({ type: 'quality', quality: q, fps });
          }}
          onSendClipboard={(text) => {
            send({ type: 'clipboard', text });
          }}
        />
      )}

      {softKeys && remoteInput && (
        <SoftKeyboardBar
          locale={locale}
          onSendText={sendText}
          onSendControl={(action, control) => send({ type: 'key', action, control })}
          onSendChord={sendChord}
          onHide={() => {
            setSoftKeys(false);
            captureRef.current?.focus();
          }}
        />
      )}
    </section>
  );
}
