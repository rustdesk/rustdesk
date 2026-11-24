import { useEffect, useMemo, useRef, useState } from "react";
import { probeCodecs, RemoteDecoder, webCodecsStatus } from "./decoder";
import { controlFromCode, isModifier } from "./keys";
import { mapToRemote } from "./mapping";
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
  const [formOpen, setFormOpen] = useState(true);
  const [rightArmed, setRightArmed] = useState(false);
  const [narrow, setNarrow] = useState(false);
  const [authRequired, setAuthRequired] = useState(false);
  const [gatewayToken, setGatewayToken] = useState("");
  const [rtt, setRtt] = useState(0);
  const [pixelFit, setPixelFit] = useState(false);

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

  useEffect(() => {
    fetch("/api/health")
      .then((response) => response.json())
      .then((body: { authRequired?: boolean }) => setAuthRequired(Boolean(body.authRequired)))
      .catch(() => setAuthRequired(false));
  }, []);

  useEffect(() => {
    const query = window.matchMedia("(max-width: 800px)");
    const apply = () => setNarrow(query.matches);
    apply();
    query.addEventListener("change", apply);
    return () => query.removeEventListener("change", apply);
  }, []);

  useEffect(() => {
    window.__rd = {
      state: phase,
      codec: stats.codec,
      width: stats.width,
      height: stats.height,
      fps: stats.fps,
      delayMs: stats.delayMs,
      bitrate: stats.bitrate,
      decoded: stats.decoded,
    };
  }, [phase, stats]);

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
        decoded: (decoderRef.current?.decoded ?? prev.decoded),
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
    setPhase("connecting");
    setDetail("Opening the gateway socket.");
    const socket = new WebSocket(wsUrl());
    socket.binaryType = "arraybuffer";
    socketRef.current = socket;
    const session = {
      token: gatewayToken,
      codecs: support,
      password,
    };
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
          setDetail(message.detail === "encrypted" ? "Encrypted session." : "Direct session.");
          if (narrow) setFormOpen(false);
        } else if (message.state === "connecting") {
          setPhase("connecting");
          setDetail(message.detail ?? "Connecting.");
        } else if (message.state === "error") {
          setPhase("error");
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
    setPhase("idle");
    setDetail("Disconnected.");
  }

  function pointerPos(event: React.PointerEvent<HTMLCanvasElement>) {
    const canvas = canvasRef.current;
    if (!canvas) return null;
    const box = canvas.getBoundingClientRect();
    const size = sizeRef.current;
    return mapToRemote(
      event.clientX - box.left,
      event.clientY - box.top,
      box.width,
      box.height,
      size.width,
      size.height,
    );
  }

  function buttonName(button: number, armed: boolean) {
    if (armed) return "right";
    if (button === 2) return "right";
    if (button === 1) return "middle";
    return "left";
  }

  function onPointerDown(event: React.PointerEvent<HTMLCanvasElement>) {
    if (phase !== "live") return;
    captureRef.current?.focus();
    const pos = pointerPos(event);
    if (!pos) return;
    const button = buttonName(event.button, rightArmed);
    gestureButton.current = button;
    setRightArmed(false);
    send({ type: "mouse", action: "down", button, x: pos.x, y: pos.y });
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function queueMove(pos: { x: number; y: number }) {
    moveRef.current = pos;
    if (moveRaf.current) return;
    moveRaf.current = requestAnimationFrame(() => {
      moveRaf.current = 0;
      const pending = moveRef.current;
      moveRef.current = null;
      if (pending) send({ type: "mouse", action: "move", x: pending.x, y: pending.y });
    });
  }

  function flushMove() {
    if (moveRaf.current) cancelAnimationFrame(moveRaf.current);
    moveRaf.current = 0;
    const pending = moveRef.current;
    moveRef.current = null;
    if (pending) send({ type: "mouse", action: "move", x: pending.x, y: pending.y });
  }

  function onPointerMove(event: React.PointerEvent<HTMLCanvasElement>) {
    if (phase !== "live") return;
    const pos = pointerPos(event);
    if (!pos) return;
    queueMove(pos);
  }

  function onPointerUp(event: React.PointerEvent<HTMLCanvasElement>) {
    if (phase !== "live") return;
    const pos = pointerPos(event);
    if (!pos) return;
    flushMove();
    send({
      type: "mouse",
      action: "up",
      button: gestureButton.current,
      x: pos.x,
      y: pos.y,
    });
  }

  function onWheel(event: React.WheelEvent<HTMLCanvasElement>) {
    if (phase !== "live") return;
    event.preventDefault();
    const notches = Math.sign(event.deltaY) || 0;
    send({ type: "mouse", action: "wheel", x: 0, y: -notches });
  }

  function sendControl(name: string, action: "down" | "up", modifiers: string[] = []) {
    send({ type: "key", action, control: name, modifiers });
  }

  function tapControl(name: string) {
    sendControl(name, "down");
    sendControl(name, "up");
  }

  function chord(parts: Array<{ control?: string; chr?: string }>) {
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

  function onCaptureKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    const control = controlFromCode(event.code);
    if (!control) {
      if ((event.ctrlKey || event.altKey || event.metaKey) && event.key.length === 1) {
        event.preventDefault();
        const modifiers = [
          event.ctrlKey ? "Control" : "",
          event.altKey ? "Alt" : "",
          event.shiftKey ? "Shift" : "",
          event.metaKey ? "Meta" : "",
        ].filter(Boolean);
        send({ type: "key", action: "down", chr: event.key, modifiers });
        send({ type: "key", action: "up", chr: event.key, modifiers });
      }
      return;
    }
    event.preventDefault();
    sendControl(control, "down");
  }

  function onCaptureKeyUp(event: React.KeyboardEvent<HTMLInputElement>) {
    const control = controlFromCode(event.code);
    if (!control) return;
    event.preventDefault();
    sendControl(control, "up");
  }

  function onBeforeInput(event: React.FormEvent<HTMLInputElement>) {
    const native = event.nativeEvent as InputEvent;
    if (native.inputType === "insertText" && native.data) {
      event.preventDefault();
      send({ type: "text", text: native.data });
    }
  }

  useEffect(() => {
    if (phase !== "live") return;
    const fromPanel = (target: EventTarget | null) =>
      target instanceof HTMLElement && Boolean(target.closest("[data-testid=connection-panel]"));
    const onDown = (event: KeyboardEvent) => {
      if (fromPanel(event.target)) return;
      if (event.target instanceof HTMLElement && event.target.matches("[data-testid=capture], [data-testid=soft-input]")) {
        return;
      }
      const control = controlFromCode(event.code);
      if (control) {
        event.preventDefault();
        sendControl(control, "down");
        return;
      }
      if (event.key.length === 1 && !event.ctrlKey && !event.altKey && !event.metaKey && !event.repeat) {
        event.preventDefault();
        send({ type: "text", text: event.key });
      }
    };
    const onUp = (event: KeyboardEvent) => {
      if (fromPanel(event.target)) return;
      if (event.target instanceof HTMLElement && event.target.matches("[data-testid=capture], [data-testid=soft-input]")) {
        return;
      }
      const control = controlFromCode(event.code);
      if (!control) return;
      event.preventDefault();
      sendControl(control, "up");
    };
    window.addEventListener("keydown", onDown);
    window.addEventListener("keyup", onUp);
    return () => {
      window.removeEventListener("keydown", onDown);
      window.removeEventListener("keyup", onUp);
    };
  }, [phase]);

  function toggleFullscreen() {
    const node = stageRef.current;
    if (!node) return;
    if (document.fullscreenElement) {
      void document.exitFullscreen();
    } else {
      void node.requestFullscreen();
    }
  }

  const displays: DisplayInfo[] = peer?.displays ?? [];
  const phaseClass = phase === "live" || phase === "connecting" || phase === "error" ? phase : "";

  return (
    <div className="app">
      <header className="topbar">
        <div className="mark">RD</div>
        <div>
          <h1>RustDesk Web Client</h1>
          <p>Control a desktop from the browser. No client install.</p>
        </div>
        <div className="metrics" data-testid="telemetry">
          <span>Codec <b data-testid="codec">{stats.codec}</b></span>
          <span>Size <b data-testid="resolution">{stats.width && stats.height ? `${stats.width}×${stats.height}` : "—"}</b></span>
          <span>FPS <b data-testid="fps">{stats.fps || "—"}</b></span>
          <span>RTT <b data-testid="delay">{(rtt || stats.delayMs) ? `${rtt || stats.delayMs} ms` : "—"}</b></span>
          <span>Frames <b data-testid="decoded">{stats.decoded}</b></span>
          <button type="button" className="ghost" data-testid="zoom" onClick={() => setPixelFit((value) => !value)}>
            {pixelFit ? "Fit" : "100%"}
          </button>
          <button type="button" className="ghost" data-testid="fullscreen" onClick={toggleFullscreen}>
            Fullscreen
          </button>
        </div>
      </header>
      <main className="workspace">
        <section className={formOpen ? "panel" : "panel collapsed"} data-testid="connection-panel">
          <h2>Connection</h2>
          <div className="modes">
            <button
              type="button"
              className={mode === "direct" ? "active" : ""}
              data-testid="mode-direct"
              onClick={() => setMode("direct")}
            >
              Direct IP
            </button>
            <button
              type="button"
              className={mode === "id" ? "active" : ""}
              data-testid="mode-id"
              onClick={() => setMode("id")}
            >
              ID / Relay
            </button>
          </div>
          {mode === "direct" ? (
            <div className="row">
              <label>
                Host
                <input data-testid="host" value={host} onChange={(e) => setHost(e.target.value)} autoComplete="off" />
              </label>
              <label>
                Port
                <input data-testid="port" value={port} onChange={(e) => setPort(e.target.value)} inputMode="numeric" />
              </label>
            </div>
          ) : (
            <>
              <label>
                Peer ID
                <input data-testid="peer-id" value={peerId} onChange={(e) => setPeerId(e.target.value)} autoComplete="off" />
              </label>
              <label>
                ID server
                <input data-testid="id-server" value={idServer} onChange={(e) => setIdServer(e.target.value)} placeholder="hbbs.example.com" />
              </label>
              <label>
                Relay server
                <input value={relayServer} onChange={(e) => setRelayServer(e.target.value)} placeholder="optional" />
              </label>
              <label>
                Server public key
                <input value={serverKey} onChange={(e) => setServerKey(e.target.value)} placeholder="base64, optional on the public network" />
              </label>
              <label className="check">
                <input type="checkbox" checked={forceRelay} onChange={(e) => setForceRelay(e.target.checked)} />
                Force relay
              </label>
            </>
          )}
          {authRequired && (
            <label>
              Gateway token
              <input
                data-testid="gateway-token"
                type="password"
                value={gatewayToken}
                onChange={(event) => setGatewayToken(event.target.value)}
                autoComplete="off"
              />
            </label>
          )}
          <label>
            Password
            <input
              data-testid="password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="off"
            />
          </label>
          <button
            type="button"
            className="primary"
            data-testid="connect"
            onClick={phase === "live" || phase === "connecting" ? disconnect : connect}
          >
            {phase === "live" || phase === "connecting" ? "Disconnect" : phase === "error" ? "Reconnect" : "Connect"}
          </button>
          <div className={`badge ${phaseClass}`} data-testid="status">
            <i />
            {phase}
          </div>
          <p className={phase === "error" ? "detail error" : "detail"} data-testid="detail">{detail}</p>
        </section>

        <section className="stage-card" ref={stageRef} data-testid="stage">
          <div className="toolbar">
            <h2>Remote display</h2>
            <div className="toolbar-actions">
              {narrow && (
                <button type="button" className="ghost" onClick={() => setFormOpen((open) => !open)}>
                  {formOpen ? "Hide connection" : "Connection"}
                </button>
              )}
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
              <button type="button" className="ghost" onClick={() => send({ type: "refresh" })} disabled={phase !== "live"}>
                Refresh
              </button>
              <button type="button" className="ghost" onClick={toggleFullscreen}>
                Fullscreen
              </button>
            </div>
          </div>
          <div className={pixelFit ? "viewport pixel" : "viewport"}>
            <canvas
              ref={canvasRef}
              className={pixelFit ? "pixel" : undefined}
              data-testid="canvas"
              width={1280}
              height={720}
              onPointerDown={onPointerDown}
              onPointerMove={onPointerMove}
              onPointerUp={onPointerUp}
              onPointerCancel={onPointerUp}
              onContextMenu={(event) => event.preventDefault()}
              onWheel={onWheel}
            />
            <input
              ref={captureRef}
              className="capture"
              data-testid="capture"
              aria-label="Remote keyboard capture"
              onKeyDown={onCaptureKeyDown}
              onKeyUp={onCaptureKeyUp}
              onBeforeInput={onBeforeInput}
              onCompositionEnd={(event) => {
                if (event.data) send({ type: "text", text: event.data });
              }}
            />
            {decoderError && (
              <div className="banner" data-testid="decoder-error">
                <strong>Video decoder</strong>
                {decoderError}
              </div>
            )}
          </div>
          <div className="quickbar" data-testid="quickbar">
            <button type="button" onClick={() => tapControl("Escape")}>Esc</button>
            <button type="button" onClick={() => tapControl("Tab")}>Tab</button>
            <button type="button" onClick={() => chord([{ control: "Control" }, { chr: "c" }])}>Ctrl+C</button>
            <button type="button" onClick={() => chord([{ control: "Control" }, { chr: "v" }])}>Ctrl+V</button>
            <button
              type="button"
              onClick={() => chord([{ control: "Control" }, { control: "Alt" }, { control: "Delete" }])}
            >
              Ctrl+Alt+Del
            </button>
            <button type="button" className={rightArmed ? "primary" : "ghost"} onClick={() => setRightArmed((v) => !v)}>
              {rightArmed ? "Right click armed" : "Right click"}
            </button>
          </div>
          <div className="softbar">
            <button
              type="button"
              data-testid="keyboard"
              onClick={() => {
                const field = document.querySelector<HTMLInputElement>("[data-testid=soft-input]");
                (narrow ? field : captureRef.current)?.focus();
              }}
            >
              Keyboard
            </button>
            <input
              data-testid="soft-input"
              placeholder="Tap Keyboard, then type"
              onKeyDown={onCaptureKeyDown}
              onKeyUp={onCaptureKeyUp}
              onBeforeInput={onBeforeInput}
              onCompositionEnd={(event) => {
                if (event.data) send({ type: "text", text: event.data });
              }}
            />
          </div>
          <div className="telemetry">
            <span>Bitrate <b data-testid="bitrate">{stats.bitrate ? formatBitrate(stats.bitrate) : "—"}</b></span>
          </div>
        </section>
      </main>
    </div>
  );
}

function formatBitrate(bitsPerSecond: number): string {
  if (bitsPerSecond >= 1_000_000) return `${(bitsPerSecond / 1_000_000).toFixed(1)} Mb/s`;
  if (bitsPerSecond >= 1000) return `${Math.round(bitsPerSecond / 1000)} kb/s`;
  return `${bitsPerSecond} b/s`;
}
