import { useEffect, useMemo, useRef, useState } from "react";
import { probeCodecs, RemoteDecoder, webCodecsStatus } from "./decoder";
import { controlFromCode, isModifier } from "./keys";
import { clampPan, fitScale, mapToRemote, zoomAround, type ViewTransform } from "./mapping";
import {
  decodeFrame,
  parseServerMessage,
  wsUrl,
  type DisplayInfo,
  type PeerMessage,
} from "./protocol";

type Phase = "idle" | "connecting" | "live" | "error";
type Mode = "direct" | "id";

interface Stats {
  codec: string;
  width: number;
  height: number;
  fps: number;
  delayMs: number;
  bitrate: number;
  decoded: number;
}

const emptyStats: Stats = {
  codec: "—",
  width: 0,
  height: 0,
  fps: 0,
  delayMs: 0,
  bitrate: 0,
  decoded: 0,
};

export default function App() {
  const codecs = useMemo(() => webCodecsStatus(), []);
  const [mode, setMode] = useState<Mode>("direct");
  const [host, setHost] = useState("127.0.0.1");
  const [port, setPort] = useState("21118");
  const [password, setPassword] = useState("");
  const [peerId, setPeerId] = useState("");
  const [idServer, setIdServer] = useState("");
  const [relayServer, setRelayServer] = useState("");
  const [serverKey, setServerKey] = useState("");
  const [forceRelay, setForceRelay] = useState(false);
  const [phase, setPhase] = useState<Phase>("idle");
  const [detail, setDetail] = useState("Enter the host and password, then connect.");
  const [peer, setPeer] = useState<PeerMessage | null>(null);
  const [display, setDisplay] = useState(0);
  const [stats, setStats] = useState<Stats>(emptyStats);
  const [decoderError, setDecoderError] = useState(codecs.ok ? "" : codecs.reason);
  const [rightArmed, setRightArmed] = useState(false);
  const [authRequired, setAuthRequired] = useState(false);
  const [gatewayToken, setGatewayToken] = useState("");
  const [rtt, setRtt] = useState(0);
  const [viewMode, setViewMode] = useState<"fit" | "original" | "custom">("fit");
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ panX: 0, panY: 0 });
  const [hand, setHand] = useState(false);
  const [touchMode, setTouchMode] = useState(false);
  const [spacePan, setSpacePan] = useState(false);
  const [handleX, setHandleX] = useState(0.5);
  const [box, setBox] = useState({ w: 0, h: 0 });
  const [menuOpen, setMenuOpen] = useState(false);
  const [connectDialog, setConnectDialog] = useState(false);
  const [softKeys, setSoftKeys] = useState(false);

  const canvasRef = useRef<HTMLCanvasElement>(null);
  const stageRef = useRef<HTMLElement>(null);
  const captureRef = useRef<HTMLInputElement>(null);
  const gestureButton = useRef("left");
  const socketRef = useRef<WebSocket | null>(null);
  const decoderRef = useRef<RemoteDecoder | null>(null);
  const sizeRef = useRef({ width: 0, height: 0 });
  const meterRef = useRef({ frames: 0, bytes: 0, at: performance.now(), delay: 0 });
  const moveRef = useRef<{ x: number; y: number } | null>(null);
  const moveRaf = useRef(0);
  const chromeRef = useRef({ menu: false, dialog: false });
  chromeRef.current = { menu: menuOpen, dialog: connectDialog || phase !== "live" };
  const viewRef = useRef({ viewMode, zoom, pan, hand, spacePan, touchMode, box });
  viewRef.current = { viewMode, zoom, pan, hand, spacePan, touchMode, box };
  const pointers = useRef(new Map<number, { x: number; y: number }>());
  const pinch = useRef<{
    dist: number;
    midX: number;
    midY: number;
    originX: number;
    originY: number;
    originDist: number;
  } | null>(null);
  const handleDrag = useRef<{ startX: number; origin: number; moved: boolean } | null>(null);
  const cursor = useRef({ x: 0, y: 0 });
  const touchTap = useRef({ x: 0, y: 0, t: 0, dragging: false });

  const remoteInput = phase === "live" && !menuOpen && !connectDialog;

  useEffect(() => {
    fetch("/api/health")
      .then((response) => response.json())
      .then((body: { authRequired?: boolean }) => setAuthRequired(Boolean(body.authRequired)))
      .catch(() => setAuthRequired(false));
  }, []);

  useEffect(() => {
    const node = stageRef.current;
    if (!node) return;
    const apply = () => setBox({ w: node.clientWidth, h: node.clientHeight });
    apply();
    const observer = new ResizeObserver(apply);
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  function currentView(): ViewTransform {
    const remoteW = sizeRef.current.width;
    const remoteH = sizeRef.current.height;
    const fit = fitScale(box.w, box.h, remoteW, remoteH);
    const scale = viewMode === "original" ? 1 : viewMode === "custom" ? Math.min(fit * 4, Math.max(fit, fit * zoom)) : fit;
    const stored = viewMode === "fit" ? { panX: (box.w - remoteW * scale) / 2, panY: (box.h - remoteH * scale) / 2 } : pan;
    return { scale, ...clampPan(box.w, box.h, remoteW, remoteH, scale, stored.panX, stored.panY) };
  }

  const view = currentView();

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
    };
    window.__rdView = {
      scale: view.scale,
      panX: view.panX,
      panY: view.panY,
      width: box.w,
      height: box.h,
    };
  }, [phase, stats, rtt, view.scale, view.panX, view.panY, box.w, box.h]);

  useEffect(() => {
    return () => {
      socketRef.current?.close();
      decoderRef.current?.close();
    };
  }, []);

  function send(payload: object) {
    const socket = socketRef.current;
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify(payload));
    }
  }

  function closeChrome() {
    chromeRef.current = { menu: false, dialog: false };
    setMenuOpen(false);
    setConnectDialog(false);
    captureRef.current?.focus();
  }

  function noteFrame(codec: string, width: number, height: number, bytes: number, delayMs: number) {
    const meter = meterRef.current;
    meter.frames += 1;
    meter.bytes += bytes;
    meter.delay = delayMs;
    const now = performance.now();
    const elapsed = now - meter.at;
    sizeRef.current = { width, height };
    if (elapsed < 500) {
      setStats((prev) => ({
        ...prev,
        codec,
        width,
        height,
        decoded: decoderRef.current?.decoded ?? prev.decoded,
      }));
      return;
    }
    const fps = Math.round((meter.frames * 1000) / elapsed);
    const bitrate = Math.round((meter.bytes * 8 * 1000) / elapsed);
    meter.frames = 0;
    meter.bytes = 0;
    meter.at = now;
    setStats({
      codec,
      width,
      height,
      fps,
      delayMs: Math.max(0, Math.round(delayMs)),
      bitrate,
      decoded: decoderRef.current?.decoded ?? 0,
    });
  }

  function ensureDecoder() {
    if (!decoderRef.current) {
      const decoder = new RemoteDecoder();
      decoder.onError = (message) => setDecoderError(message);
      decoder.onNeedRefresh = () => send({ type: "refresh" });
      decoder.onOutput = (width, height) => {
        sizeRef.current = { width, height };
      };
      decoderRef.current = decoder;
    }
    return decoderRef.current;
  }

  async function connect() {
    if (!codecs.ok) {
      setPhase("error");
      setDetail(codecs.reason);
      setDecoderError(codecs.reason);
      return;
    }
    const support = await probeCodecs();
    if (!support.vp8 && !support.vp9 && !support.h264 && !support.h265 && !support.av1) {
      setPhase("error");
      setDetail("This browser cannot decode VP8, VP9, H.264, HEVC, or AV1.");
      return;
    }
    socketRef.current?.close();
    decoderRef.current?.close();
    decoderRef.current = null;
    setPeer(null);
    setStats(emptyStats);
    setRtt(0);
    setDecoderError("");
    setMenuOpen(false);
    setPhase("connecting");
    setDetail("Opening the gateway socket.");
    const socket = new WebSocket(wsUrl());
    socket.binaryType = "arraybuffer";
    socketRef.current = socket;
    const session = { token: gatewayToken, codecs: support, password };
    socket.onopen = () => {
      setDetail("Authenticating with the host.");
      if (mode === "id") {
        send({
          type: "connect",
          mode: "id",
          peerId,
          idServer,
          relayServer,
          key: serverKey,
          forceRelay,
          ...session,
        });
      } else {
        send({
          type: "connect",
          mode: "direct",
          host,
          port: Number(port) || 21118,
          ...session,
        });
      }
    };
    socket.onmessage = (event) => {
      if (typeof event.data !== "string") {
        const packet = decodeFrame(event.data as ArrayBuffer);
        if (!packet) return;
        ensureDecoder().push(packet, canvasRef.current);
        const delay = packet.gatewayUnixMs ? Date.now() - packet.gatewayUnixMs : 0;
        noteFrame(packet.codecName, packet.width, packet.height, packet.data.byteLength, delay);
        return;
      }
      const message = parseServerMessage(event.data);
      if (!message) return;
      if (message.type === "status") {
        if (message.state === "live") {
          setPhase("live");
          setConnectDialog(false);
          setMenuOpen(false);
          setDetail(message.detail === "encrypted" ? "Encrypted session." : "Direct session.");
        } else if (message.state === "connecting") {
          setPhase("connecting");
          setDetail(message.detail ?? "Connecting.");
        } else if (message.state === "error") {
          setPhase("error");
          setConnectDialog(true);
          setDetail(message.detail ?? "The host rejected the session.");
        } else if (message.state === "closed") {
          setPhase("idle");
          setDetail("Disconnected.");
        }
      } else if (message.type === "peer") {
        setPeer(message);
        setDisplay(message.currentDisplay);
        const current = message.displays[message.currentDisplay] ?? message.displays[0];
        if (current) {
          sizeRef.current = { width: current.width, height: current.height };
          setStats((prev) => ({ ...prev, width: current.width, height: current.height }));
        }
      } else if (message.type === "display") {
        setDisplay(message.display);
        if (message.width > 0 && message.height > 0) {
          sizeRef.current = { width: message.width, height: message.height };
        }
      } else if (message.type === "error") {
        setDecoderError(message.message);
        setDetail(message.message);
      } else if (message.type === "rtt") {
        setRtt(message.ms);
      }
    };
    socket.onerror = () => {
      setPhase("error");
      setConnectDialog(true);
      setDetail("Cannot reach the gateway. Start it on this machine, then connect again.");
    };
    socket.onclose = () => {
      setPhase((prev) => (prev === "error" ? prev : "idle"));
    };
  }

  function disconnect() {
    send({ type: "disconnect" });
    socketRef.current?.close();
    socketRef.current = null;
    setMenuOpen(false);
    setConnectDialog(false);
    setSoftKeys(false);
    setPhase("idle");
    setDetail("Disconnected.");
  }

  function stagePoint(event: { clientX: number; clientY: number }) {
    const rect = stageRef.current?.getBoundingClientRect();
    if (!rect) return null;
    return { x: event.clientX - rect.left, y: event.clientY - rect.top };
  }

  function pointerPos(event: { clientX: number; clientY: number }) {
    const local = stagePoint(event);
    if (!local) return null;
    return mapToRemote(local.x, local.y, sizeRef.current.width, sizeRef.current.height, currentView());
  }

  function setScale(nextScale: number, anchorX: number, anchorY: number) {
    const remoteW = sizeRef.current.width;
    const remoteH = sizeRef.current.height;
    const fit = fitScale(box.w, box.h, remoteW, remoteH);
    const clampedScale = Math.min(fit * 4, Math.max(Math.min(fit, 1), nextScale));
    const current = currentView();
    const shifted = zoomAround(anchorX, anchorY, current.scale, clampedScale, current.panX, current.panY);
    const nextPan = clampPan(box.w, box.h, remoteW, remoteH, clampedScale, shifted.panX, shifted.panY);
    setViewMode(Math.abs(clampedScale - fit) < 0.01 ? "fit" : Math.abs(clampedScale - 1) < 0.01 ? "original" : "custom");
    setZoom(fit === 0 ? 1 : clampedScale / fit);
    setPan(nextPan);
  }

  function buttonName(button: number, armed: boolean) {
    if (armed) return "right";
    if (button === 2) return "right";
    if (button === 1) return "middle";
    return "left";
  }

  function onPointerDown(event: React.PointerEvent<HTMLElement>) {
    if (!remoteInput) return;
    const local = stagePoint(event);
    if (!local) return;
    pointers.current.set(event.pointerId, local);
    event.currentTarget.setPointerCapture(event.pointerId);
    if (pointers.current.size >= 2) {
      pinch.current = pinchSnapshot();
      return;
    }
    const panning = hand || spacePan;
    if (panning) return;
    if (event.pointerType === "touch" && !touchMode) {
      touchTap.current = { x: local.x, y: local.y, t: performance.now(), dragging: false };
      return;
    }
    const pos = pointerPos(event);
    if (!pos) return;
    const button = buttonName(event.button, rightArmed);
    gestureButton.current = button;
    setRightArmed(false);
    send({ type: "mouse", action: "down", button, x: pos.x, y: pos.y });
  }

  function pinchSnapshot() {
    const pts = [...pointers.current.values()];
    if (pts.length < 2) return null;
    const dx = pts[0].x - pts[1].x;
    const dy = pts[0].y - pts[1].y;
    const midX = (pts[0].x + pts[1].x) / 2;
    const midY = (pts[0].y + pts[1].y) / 2;
    const dist = Math.hypot(dx, dy) || 1;
    return { dist, midX, midY, originX: midX, originY: midY, originDist: dist };
  }

  function queueMove(pos: { x: number; y: number }) {
    moveRef.current = pos;
    if (moveRaf.current) return;
    moveRaf.current = requestAnimationFrame(() => {
      moveRaf.current = 0;
      const pending = moveRef.current;
      moveRef.current = null;
      if (pending && !chromeRef.current.menu && !chromeRef.current.dialog) {
        send({ type: "mouse", action: "move", x: pending.x, y: pending.y });
      }
    });
  }

  function flushMove() {
    if (moveRaf.current) cancelAnimationFrame(moveRaf.current);
    moveRaf.current = 0;
    const pending = moveRef.current;
    moveRef.current = null;
    if (pending && remoteInput) send({ type: "mouse", action: "move", x: pending.x, y: pending.y });
  }

  function onPointerMove(event: React.PointerEvent<HTMLElement>) {
    if (!remoteInput) return;
    const local = stagePoint(event);
    if (!local) return;
    const prev = pointers.current.get(event.pointerId) ?? local;
    pointers.current.set(event.pointerId, local);
    const dx = local.x - prev.x;
    const dy = local.y - prev.y;
    if (pointers.current.size >= 2 && pinch.current) {
      const next = pinchSnapshot();
      if (!next) return;
      const scaleFactor = next.dist / pinch.current.dist;
      if (Math.abs(scaleFactor - 1) > 0.02) {
        setScale(currentView().scale * scaleFactor, next.midX, next.midY);
      } else {
        const dx = next.midX - pinch.current.midX;
        const dy = next.midY - pinch.current.midY;
        const current = currentView();
        const remoteW = sizeRef.current.width;
        const remoteH = sizeRef.current.height;
        setViewMode("custom");
        setZoom(fitScale(box.w, box.h, remoteW, remoteH) === 0 ? 1 : current.scale / fitScale(box.w, box.h, remoteW, remoteH));
        setPan(clampPan(box.w, box.h, remoteW, remoteH, current.scale, current.panX + dx, current.panY + dy));
      }
      pinch.current = { ...next, originX: pinch.current.originX, originY: pinch.current.originY, originDist: pinch.current.originDist };
      return;
    }
    if (hand || spacePan) {
      const current = currentView();
      const remoteW = sizeRef.current.width;
      const remoteH = sizeRef.current.height;
      setPan(clampPan(box.w, box.h, remoteW, remoteH, current.scale, current.panX + dx, current.panY + dy));
      if (current.scale > fitScale(box.w, box.h, remoteW, remoteH) + 0.01) setViewMode("custom");
      return;
    }
    if (event.pointerType === "touch" && !touchMode) {
      const moved = Math.hypot(local.x - touchTap.current.x, local.y - touchTap.current.y);
      cursor.current.x = Math.min(sizeRef.current.width - 1, Math.max(0, cursor.current.x + dx / currentView().scale));
      cursor.current.y = Math.min(sizeRef.current.height - 1, Math.max(0, cursor.current.y + dy / currentView().scale));
      const pos = { x: Math.round(cursor.current.x), y: Math.round(cursor.current.y) };
      if (moved > 8 && touchTap.current.dragging) {
        queueMove(pos);
      }
      return;
    }
    const pos = pointerPos(event);
    if (!pos) return;
    queueMove(pos);
  }

  function onPointerUp(event: React.PointerEvent<HTMLElement>) {
    const local = stagePoint(event);
    const wasPinch = pointers.current.size >= 2;
    pointers.current.delete(event.pointerId);
    if (pointers.current.size < 2) pinch.current = null;
    if (!remoteInput || !local) return;
    if (wasPinch) {
      if (pinchWasTap()) {
        const pos = pointerPos(event);
        if (pos) {
          send({ type: "mouse", action: "down", button: "right", x: pos.x, y: pos.y });
          send({ type: "mouse", action: "up", button: "right", x: pos.x, y: pos.y });
        }
      }
      return;
    }
    if (hand || spacePan) return;
    if (event.pointerType === "touch" && !touchMode) {
      const moved = Math.hypot(local.x - touchTap.current.x, local.y - touchTap.current.y);
      const pos = { x: Math.round(cursor.current.x), y: Math.round(cursor.current.y) };
      if (touchTap.current.dragging) {
        flushMove();
        send({ type: "mouse", action: "up", button: "left", x: pos.x, y: pos.y });
        touchTap.current.dragging = false;
        return;
      }
      if (moved < 8) {
        const now = performance.now();
        if (now - touchTap.current.t < 280) {
          touchTap.current.dragging = true;
          send({ type: "mouse", action: "down", button: "left", x: pos.x, y: pos.y });
          return;
        }
        send({ type: "mouse", action: "down", button: "left", x: pos.x, y: pos.y });
        send({ type: "mouse", action: "up", button: "left", x: pos.x, y: pos.y });
      }
      return;
    }
    const pos = pointerPos(event);
    if (!pos) return;
    flushMove();
    send({ type: "mouse", action: "up", button: gestureButton.current, x: pos.x, y: pos.y });
  }

  function pinchWasTap() {
    const start = pinch.current;
    if (!start) return false;
    const midShift = Math.hypot(start.midX - start.originX, start.midY - start.originY);
    return midShift < 12 && Math.abs(start.dist - start.originDist) < 12;
  }

  function onWheel(event: React.WheelEvent<HTMLElement>) {
    if (!remoteInput) return;
    event.preventDefault();
    const local = stagePoint(event);
    if (event.ctrlKey || event.metaKey || event.altKey) {
      if (!local) return;
      const factor = event.deltaY < 0 ? 1.1 : 1 / 1.1;
      setScale(currentView().scale * factor, local.x, local.y);
      return;
    }
    const notches = Math.sign(event.deltaY) || 0;
    send({ type: "mouse", action: "wheel", x: Math.sign(event.deltaX) || 0, y: -notches });
  }

  function tapControl(name: string) {
    if (chromeRef.current.menu || chromeRef.current.dialog) return;
    send({ type: "key", action: "down", control: name });
    send({ type: "key", action: "up", control: name });
  }

  function chord(parts: Array<{ control?: string; chr?: string }>) {
    if (chromeRef.current.menu || chromeRef.current.dialog) return;
    const modifiers = parts.flatMap((part) => (part.control && isModifier(part.control) ? [part.control] : []));
    for (const part of parts) {
      if (part.control) send({ type: "key", action: "down", control: part.control, modifiers });
      if (part.chr) send({ type: "key", action: "down", chr: part.chr, modifiers });
    }
    for (const part of [...parts].reverse()) {
      if (part.chr) send({ type: "key", action: "up", chr: part.chr, modifiers });
      if (part.control) send({ type: "key", action: "up", control: part.control, modifiers });
    }
  }

  useEffect(() => {
    const inField = (target: EventTarget | null) =>
      target instanceof HTMLElement &&
      Boolean(target.closest("[data-testid=connection-panel], [data-testid=session-menu], [data-testid=softbar]"));
    const onDown = (event: KeyboardEvent) => {
      const blocked = chromeRef.current.menu || chromeRef.current.dialog;
      if (blocked) {
        if (event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          setMenuOpen(false);
          setConnectDialog(false);
          return;
        }
        if (inField(event.target)) return;
        event.preventDefault();
        return;
      }
      if (phase !== "live") return;
      if (inField(event.target)) return;
      if (event.code === "Space") {
        event.preventDefault();
        setSpacePan(true);
        return;
      }
      const control = controlFromCode(event.code);
      if (control) {
        event.preventDefault();
        send({ type: "key", action: "down", control });
        return;
      }
      if (event.key.length === 1 && !event.ctrlKey && !event.altKey && !event.metaKey && !event.repeat) {
        event.preventDefault();
        send({ type: "text", text: event.key });
      }
    };
    const onUp = (event: KeyboardEvent) => {
      if (event.code === "Space") {
        setSpacePan(false);
        return;
      }
      if (chromeRef.current.menu || chromeRef.current.dialog || phase !== "live") return;
      if (inField(event.target)) return;
      const control = controlFromCode(event.code);
      if (!control) return;
      event.preventDefault();
      send({ type: "key", action: "up", control });
    };
    window.addEventListener("keydown", onDown, true);
    window.addEventListener("keyup", onUp, true);
    return () => {
      window.removeEventListener("keydown", onDown, true);
      window.removeEventListener("keyup", onUp, true);
    };
  }, [phase]);

  function toggleFullscreen() {
    const node = stageRef.current;
    if (!node) return;
    if (document.fullscreenElement) void document.exitFullscreen();
    else void node.requestFullscreen();
  }

  const displays: DisplayInfo[] = peer?.displays ?? [];
  const showConnect = phase !== "live" || connectDialog;
  const phaseClass = phase === "live" || phase === "connecting" || phase === "error" ? phase : "";
  const shownDelay = rtt || stats.delayMs;

  return (
    <section
      className={hand || spacePan ? "stage hand" : "stage"}
      ref={stageRef}
      data-testid="stage"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onContextMenu={(event) => event.preventDefault()}
      onWheel={onWheel}
    >
      <canvas
        ref={canvasRef}
        data-testid="canvas"
        width={Math.max(1, sizeRef.current.width || 1280)}
        height={Math.max(1, sizeRef.current.height || 720)}
        style={{
          left: view.panX,
          top: view.panY,
          width: Math.max(1, sizeRef.current.width * view.scale),
          height: Math.max(1, sizeRef.current.height * view.scale),
        }}
      />
      <input ref={captureRef} className="capture" data-testid="capture" aria-hidden="true" tabIndex={-1} />

      {phase === "live" && !showConnect && !menuOpen && (
        <button
          type="button"
          className={decoderError ? "pull warn" : "pull"}
          data-testid="session-handle"
          style={{ left: `${handleX * 100}%` }}
          onPointerDown={(event) => {
            event.stopPropagation();
            handleDrag.current = { startX: event.clientX, origin: handleX, moved: false };
            event.currentTarget.setPointerCapture(event.pointerId);
          }}
          onPointerMove={(event) => {
            const drag = handleDrag.current;
            const stage = stageRef.current;
            if (!drag || !stage) return;
            const width = stage.getBoundingClientRect().width || 1;
            if (Math.abs(event.clientX - drag.startX) > 4) drag.moved = true;
            const next = drag.origin + (event.clientX - drag.startX) / width;
            setHandleX(Math.min(0.92, Math.max(0.08, next)));
          }}
          onPointerUp={(event) => {
            event.stopPropagation();
            if (!handleDrag.current?.moved) setMenuOpen(true);
            handleDrag.current = null;
          }}
        >
          Session
        </button>
      )}

      {showConnect && (
        <div className="scrim" onPointerDown={(event) => event.stopPropagation()} onMouseDown={() => phase === "live" && closeChrome()}>
          <form
            className="sheet"
            data-testid="connection-panel"
            onMouseDown={(event) => event.stopPropagation()}
            onSubmit={(event) => {
              event.preventDefault();
              void connect();
            }}
          >
            <div className="sheet-head">
              <h2>Connection</h2>
              {phase === "live" && (
                <button type="button" className="ghost" onClick={closeChrome}>
                  Close
                </button>
              )}
            </div>
            <div className="modes">
              <button type="button" className={mode === "direct" ? "active" : ""} data-testid="mode-direct" onClick={() => setMode("direct")}>
                Direct IP
              </button>
              <button type="button" className={mode === "id" ? "active" : ""} data-testid="mode-id" onClick={() => setMode("id")}>
                ID / Relay
              </button>
            </div>
            {mode === "direct" ? (
              <div className="row">
                <label>
                  Host
                  <input data-testid="host" value={host} onChange={(event) => setHost(event.target.value)} autoComplete="off" />
                </label>
                <label>
                  Port
                  <input data-testid="port" value={port} onChange={(event) => setPort(event.target.value)} inputMode="numeric" />
                </label>
              </div>
            ) : (
              <>
                <label>
                  Peer ID
                  <input data-testid="peer-id" value={peerId} onChange={(event) => setPeerId(event.target.value)} autoComplete="off" />
                </label>
                <label>
                  ID server
                  <input data-testid="id-server" value={idServer} onChange={(event) => setIdServer(event.target.value)} placeholder="hbbs.example.com" />
                </label>
                <label>
                  Relay server
                  <input value={relayServer} onChange={(event) => setRelayServer(event.target.value)} placeholder="optional" />
                </label>
                <label>
                  Server public key
                  <input value={serverKey} onChange={(event) => setServerKey(event.target.value)} placeholder="base64, optional" />
                </label>
                <label className="check">
                  <input type="checkbox" checked={forceRelay} onChange={(event) => setForceRelay(event.target.checked)} />
                  Force relay
                </label>
              </>
            )}
            {authRequired && (
              <label>
                Gateway token
                <input data-testid="gateway-token" type="password" value={gatewayToken} onChange={(event) => setGatewayToken(event.target.value)} autoComplete="off" />
              </label>
            )}
            <label>
              Password
              <input data-testid="password" type="password" value={password} onChange={(event) => setPassword(event.target.value)} autoComplete="off" />
            </label>
            <div className="actions" style={{ marginTop: 12 }}>
              <button type="submit" className="primary" data-testid="connect" disabled={phase === "connecting"}>
                {phase === "connecting" ? "Connecting" : phase === "error" || phase === "live" ? "Reconnect" : "Connect"}
              </button>
              {phase === "live" && (
                <button type="button" className="ghost" data-testid="disconnect" onClick={disconnect}>
                  Disconnect
                </button>
              )}
            </div>
            <div className={`badge ${phaseClass}`} data-testid="status">
              <i />
              {phase}
            </div>
            <p className={phase === "error" ? "detail error" : "detail"} data-testid="detail">{detail}</p>
            {decoderError && phase !== "live" && (
              <p className="detail error" data-testid="decoder-error">{decoderError}</p>
            )}
          </form>
        </div>
      )}

      {menuOpen && phase === "live" && !connectDialog && (
        <div className="scrim" data-testid="session-backdrop" onPointerDown={(event) => event.stopPropagation()} onMouseDown={closeChrome}>
          <div className="sheet" data-testid="session-menu" onPointerDown={(event) => event.stopPropagation()} onMouseDown={(event) => event.stopPropagation()}>
            <div className="sheet-head">
              <h2>Session</h2>
              <button type="button" className="ghost" data-testid="session-close" onClick={closeChrome}>
                Close
              </button>
            </div>
            <div className={`badge ${phaseClass}`} data-testid="status">
              <i />
              {phase}
            </div>
            <p className="detail" data-testid="detail">
              {peer ? `${peer.hostname || peer.username || "Host"} · ${peer.platform || "remote"}` : detail}
            </p>
            <div className="actions" style={{ marginTop: 8 }}>
              <button type="button" className="ghost" onClick={() => { setMenuOpen(false); setConnectDialog(true); }}>
                Connection
              </button>
              <button type="button" className="ghost" data-testid="disconnect" onClick={disconnect}>
                Disconnect
              </button>
            </div>

            <h3>View</h3>
            <div className="actions">
              <button type="button" className={viewMode === "fit" ? "primary" : "ghost"} onClick={() => setViewMode("fit")}>Fit</button>
              <button type="button" className={viewMode === "original" ? "primary" : "ghost"} onClick={() => { setViewMode("original"); setPan(clampPan(box.w, box.h, sizeRef.current.width, sizeRef.current.height, 1, (box.w - sizeRef.current.width) / 2, (box.h - sizeRef.current.height) / 2)); }}>100%</button>
              <button type="button" className="ghost" data-testid="zoom-out" onClick={() => setScale(view.scale / 1.15, box.w / 2, box.h / 2)}>−</button>
              <button type="button" className="ghost" data-testid="zoom-in" onClick={() => setScale(view.scale * 1.15, box.w / 2, box.h / 2)}>+</button>
              <span data-testid="zoom">{Math.round((fitScale(box.w, box.h, sizeRef.current.width, sizeRef.current.height) === 0 ? 1 : view.scale / fitScale(box.w, box.h, sizeRef.current.width, sizeRef.current.height)) * 100)}%</span>
              <button type="button" className={hand ? "primary" : "ghost"} data-testid="pan" onClick={() => setHand((value) => !value)}>{hand ? "Panning" : "Pan"}</button>
              <button type="button" className={touchMode ? "primary" : "ghost"} data-testid="touch-mode" onClick={() => setTouchMode((value) => !value)}>{touchMode ? "Touch" : "Mouse"}</button>
            </div>
            <div className="actions">
              <select
                data-testid="display"
                value={display}
                disabled={displays.length === 0}
                onChange={(event) => {
                  const next = Number(event.target.value);
                  setDisplay(next);
                  send({ type: "switchDisplay", display: next });
                }}
              >
                {displays.length === 0 && <option value={0}>Display</option>}
                {displays.map((item) => (
                  <option key={item.index} value={item.index}>
                    {item.name || `Display ${item.index + 1}`} {item.width}×{item.height}
                  </option>
                ))}
              </select>
              <button type="button" className="ghost" onClick={() => send({ type: "refresh" })}>Refresh</button>
              <button type="button" className="ghost" data-testid="fullscreen" onClick={toggleFullscreen}>Fullscreen</button>
            </div>

            <h3>Input</h3>
            <div className="quickbar" data-testid="quickbar">
              <button type="button" onClick={() => { closeChrome(); tapControl("Escape"); }}>Esc</button>
              <button type="button" onClick={() => { closeChrome(); tapControl("Tab"); }}>Tab</button>
              <button type="button" onClick={() => { closeChrome(); chord([{ control: "Control" }, { chr: "c" }]); }}>Ctrl+C</button>
              <button type="button" onClick={() => { closeChrome(); chord([{ control: "Control" }, { chr: "v" }]); }}>Ctrl+V</button>
              <button type="button" onClick={() => { closeChrome(); chord([{ control: "Control" }, { control: "Alt" }, { control: "Delete" }]); }}>
                Ctrl+Alt+Del
              </button>
              <button type="button" className={rightArmed ? "primary" : "ghost"} onClick={() => setRightArmed((value) => !value)}>
                {rightArmed ? "Right click armed" : "Right click"}
              </button>
              <button
                type="button"
                data-testid="keyboard"
                className={softKeys ? "primary" : "ghost"}
                onClick={() => {
                  setSoftKeys((value) => !value);
                  closeChrome();
                }}
              >
                Keyboard
              </button>
            </div>

            <h3>Stats</h3>
            <div className="stats" data-testid="telemetry">
              <div>Codec <b data-testid="codec">{stats.codec}</b></div>
              <div>Size <b data-testid="resolution">{stats.width && stats.height ? `${stats.width}×${stats.height}` : "—"}</b></div>
              <div>FPS <b data-testid="fps">{stats.fps || "—"}</b></div>
              <div>RTT <b data-testid="delay">{shownDelay ? `${shownDelay} ms` : "—"}</b></div>
              <div>Bitrate <b data-testid="bitrate">{stats.bitrate ? formatBitrate(stats.bitrate) : "—"}</b></div>
              <div>Frames <b data-testid="decoded">{stats.decoded}</b></div>
            </div>
            {decoderError && <p className="detail error" data-testid="decoder-error">{decoderError}</p>}
          </div>
        </div>
      )}

      {softKeys && remoteInput && (
        <div className="softbar" data-testid="softbar" onPointerDown={(event) => event.stopPropagation()}>
          <input
            data-testid="soft-input"
            placeholder="Type to the remote desktop"
            onKeyDown={(event) => {
              const control = controlFromCode(event.code);
              if (!control) return;
              event.preventDefault();
              send({ type: "key", action: "down", control });
            }}
            onKeyUp={(event) => {
              const control = controlFromCode(event.code);
              if (!control) return;
              event.preventDefault();
              send({ type: "key", action: "up", control });
            }}
            onBeforeInput={(event) => {
              const native = event.nativeEvent as InputEvent;
              if (native.inputType === "insertText" && native.data) {
                event.preventDefault();
                send({ type: "text", text: native.data });
              }
            }}
            onCompositionEnd={(event) => {
              if (event.data) send({ type: "text", text: event.data });
            }}
          />
          <button type="button" className="ghost" onClick={() => setSoftKeys(false)}>Hide</button>
        </div>
      )}
    </section>
  );
}

function formatBitrate(bitsPerSecond: number): string {
  if (bitsPerSecond >= 1_000_000) return `${(bitsPerSecond / 1_000_000).toFixed(1)} Mb/s`;
  if (bitsPerSecond >= 1000) return `${Math.round(bitsPerSecond / 1000)} kb/s`;
  return `${bitsPerSecond} b/s`;
}
