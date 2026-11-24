import { PasteKeysButton } from './PasteKeysButton';
import React, { useState } from 'react';
import {
  Activity,
  Keyboard,
  Monitor,
  Power,
  RefreshCw,
  Sliders,
  Sparkles,
  X,
} from 'lucide-react';
import { formatConnectionError } from '../lib/errors';
import { statusLabel, t, type Locale } from '../lib/i18n';
import type { ViewMode } from '../lib/mapping';
import { type PeerMessage, type QualityLevel } from '../lib/protocol';
import type { ConnectionPhase } from './RustDeskShell';

import type { PreferredCodec } from '../lib/displayPrefs';
import type { StreamActivityState } from '../lib/stats';

export interface SessionStats {
  codec: string;
  width: number;
  height: number;
  fps: number;
  rxFps: number;
  delayMs: number;
  bitrate: number;
  decoded: number;
  dropped: number;
  streamState: StreamActivityState;
}

type MenuTab = 'control' | 'input' | 'display' | 'session';

interface SessionMenuProps {
  locale: Locale;
  onPasteKeys: () => Promise<boolean>;
  pasteDisabled: boolean;
  audioEnabled: boolean;
  audioIssue: string | null;
  onToggleAudio: () => void;
  mobileLayout: boolean;
  cursorEmbedded: boolean;
  followWindow: boolean;
  onToggleFollowWindow: () => void;
  enlargeCursor: boolean;
  onToggleEnlargeCursor: () => void;
  phase: ConnectionPhase;
  peer: PeerMessage | null;
  detail: string;
  decoderError: string;
  display: number;
  viewMode: ViewMode;
  zoomPercent: number;
  hand: boolean;
  touchMode: boolean;
  rightArmed: boolean;
  softKeys: boolean;
  viewOnly: boolean;
  followRemoteCursor: boolean;
  onToggleFollowCursor: () => void;
  sizes: { width: number; height: number }[];
  onResolution: (width: number, height: number) => void;
  showRemoteCursor: boolean;
  diagnosticsOpen: boolean;
  stats: SessionStats;
  rtt: number;
  quality: QualityLevel;
  targetFps: number;
  preferredCodec: PreferredCodec;
  remoteClipboard: string;
  onClose: () => void;
  onOpenConnection: () => void;
  onDisconnect: () => void;
  onToggleDiagnostics: () => void;
  onToggleViewOnly: () => void;
  onToggleRemoteCursor: () => void;
  onFit: () => void;
  onOriginal: () => void;
  onZoomOut: () => void;
  onZoomIn: () => void;
  onToggleHand: () => void;
  onToggleTouchMode: () => void;
  onSwitchDisplay: (index: number) => void;
  onRefresh: () => void;
  onQuickKey: (name: 'Esc' | 'Tab' | 'Ctrl+C' | 'Ctrl+V' | 'Ctrl+Alt+Del' | 'Lock') => void;
  onRightClick: () => void;
  onToggleKeyboard: () => void;
  onChangeQuality: (quality: QualityLevel, fps: number, codec?: PreferredCodec) => void;
  onSendClipboard: (text: string) => void;
}

export function SessionMenu({
  locale,
  onPasteKeys,
  pasteDisabled,
  audioEnabled,
  audioIssue,
  onToggleAudio,
  mobileLayout,
  cursorEmbedded,
  followWindow,
  onToggleFollowWindow,
  enlargeCursor,
  onToggleEnlargeCursor,
  phase,
  peer,
  detail,
  decoderError,
  display,
  viewMode,
  zoomPercent,
  hand,
  touchMode,
  rightArmed,
  softKeys,
  viewOnly,
  followRemoteCursor, onToggleFollowCursor, sizes, onResolution,
  showRemoteCursor,
  diagnosticsOpen,
  stats,
  rtt,
  quality,
  targetFps,
  preferredCodec,
  remoteClipboard,
  onClose,
  onOpenConnection,
  onDisconnect,
  onToggleDiagnostics,
  onToggleViewOnly,
  onToggleRemoteCursor,
  onFit,
  onOriginal,
  onZoomOut,
  onZoomIn,
  onToggleHand,
  onToggleTouchMode,
  onSwitchDisplay,
  onRefresh,
  onQuickKey,
  onRightClick,
  onToggleKeyboard,
  onChangeQuality,
  onSendClipboard,
}: SessionMenuProps) {
  const [activeTab, setActiveTab] = useState<MenuTab>('control');
  const [clipDraft, setClipDraft] = useState('');
  const [clipNotice, setClipNotice] = useState('');

  const displays = peer?.displays ?? [];
  const hasMultipleDisplays = displays.length > 1;
  const hostTitle = peer?.hostname || peer?.username || 'Host';
  const platformLabel = peer?.platform || 'remote';
  const linkLabel = peer?.encrypted ? t(locale, 'linkEncrypted') : t(locale, 'linkDirect');
  const displayRtt = rtt || stats.delayMs;

  const handleSendClip = (e: React.FormEvent) => {
    e.preventDefault();
    if (!clipDraft.trim()) return;
    onSendClipboard(clipDraft);
    setClipDraft('');
    setClipNotice(t(locale, 'clipSent'));
    window.setTimeout(() => setClipNotice(''), 2000);
  };

  const handleCopyRemote = async () => {
    if (!remoteClipboard) return;
    try {
      await navigator.clipboard.writeText(remoteClipboard);
      setClipNotice(t(locale, 'clipCopied'));
    } catch {
      setClipNotice(t(locale, 'clipDenied'));
    }
    window.setTimeout(() => setClipNotice(''), 2000);
  };

  const tabs: Array<{ id: MenuTab; label: string; icon: React.ReactNode }> = [
    { id: 'control', label: t(locale, 'menuTabControl'), icon: <Sliders size={14} /> },
    { id: 'input', label: t(locale, 'menuTabInput'), icon: <Keyboard size={14} /> },
    { id: 'display', label: t(locale, 'menuTabDisplay'), icon: <Sparkles size={14} /> },
    { id: 'session', label: t(locale, 'menuTabSession'), icon: <Activity size={14} /> },
  ];

  return (
    <div
      className="scrim"
      data-testid="session-backdrop"
      onPointerDown={(e) => {
        e.stopPropagation();
        if (e.target === e.currentTarget) {
          onClose();
        }
      }}
    >
      <div
        className="sheet sheet-menu"
        data-testid="session-menu"
        role="dialog"
        aria-label={t(locale, 'session')}
        onPointerDown={(e) => e.stopPropagation()}
      >
        <div
          className="sm:hidden w-10 h-1 rounded-full bg-[var(--line)] mx-auto mb-2.5"
          aria-hidden="true"
        />

        {/* Header */}
        <div className="flex items-center justify-between gap-2 pb-3 border-b border-[var(--rd-divider)]">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <h2 className="text-base font-semibold text-[var(--ink)] m-0 truncate">
                {hostTitle}
              </h2>
              <span className={`rd-status ${phase}`} data-testid="status">
                {statusLabel(locale, phase)}
              </span>
            </div>
            <p className="text-xs text-[var(--ink-soft)] m-0 mt-0.5 truncate" data-testid="detail">
              {platformLabel} · {linkLabel}
              {displayRtt > 0 ? ` · ${displayRtt} ms` : ''}
            </p>
          </div>
          <div className="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              className="rd-btn"
              style={{ minWidth: 36, minHeight: 36, padding: 6 }}
              data-testid="session-close"
              aria-label={t(locale, 'close')}
              data-tooltip={t(locale, 'close')}
              data-tooltip-pos="bottom-end"
              onClick={onClose}
            >
              <X size={16} />
            </button>
          </div>
        </div>

        {/* Categorized 4-Tab Bar */}
        <div
          className="rd-session-tabs my-3"
          role="tablist"
          aria-label={t(locale, 'session')}
        >
          {tabs.map((tab) => (
            <button
              key={tab.id}
              type="button"
              role="tab"
              aria-selected={activeTab === tab.id}
              data-testid={`menu-tab-${tab.id}`}
              className={`rd-session-tab${activeTab === tab.id ? ' active' : ''}`}
              onClick={() => setActiveTab(tab.id)}
            >
              {tab.icon}
              <span>{tab.label}</span>
            </button>
          ))}
        </div>

        {/* Tab 1: Control (View, Displays, Input Mode, Quick Actions) */}
        {activeTab === 'control' && (
          <div className="flex flex-col gap-3.5" role="tabpanel">
            {hasMultipleDisplays && (
              <div className="session-group" data-testid="display-switcher">
                <div className="flex items-center justify-between gap-2 mb-2">
                  <span className="text-xs font-semibold text-[var(--ink-soft)]">
                    {t(locale, 'displays')}
                  </span>
                  <span className="text-xs font-mono-tabular text-[var(--ink-soft)]">
                    {displays.length} {t(locale, 'monitorsCount')}
                  </span>
                </div>
                <div
                  className="grid grid-cols-1 sm:grid-cols-2 gap-2"
                  role="group"
                  aria-label={t(locale, 'displays')}
                  data-testid="display"
                >
                  {displays.map((d) => {
                    const isSelected = display === d.index;
                    const isOffline = d.online === false;
                    const label = d.name || `${t(locale, 'displayFallback')} ${d.index + 1}`;
                    return (
                      <button
                        key={d.index}
                        type="button"
                        disabled={isOffline}
                        aria-pressed={isSelected}
                        data-testid={`display-option-${d.index}`}
                        className={`display-card${isSelected ? ' active' : ''}`}
                        onClick={() => onSwitchDisplay(d.index)}
                      >
                        <div className="flex items-center justify-between gap-2 w-full">
                          <span className="inline-flex items-center gap-1.5 text-xs font-semibold text-[var(--ink)] min-w-0">
                            <Monitor
                              size={15}
                              className={
                                isSelected
                                  ? 'text-[var(--accent)] shrink-0'
                                  : 'text-[var(--ink-soft)] shrink-0'
                              }
                            />
                            <span className="font-mono-tabular">#{d.index + 1}</span>
                          </span>
                          <span
                            className={`text-[11px] font-medium ${
                              isOffline
                                ? 'text-[var(--bad)]'
                                : isSelected
                                  ? 'text-[var(--accent)]'
                                  : 'text-[var(--ink-soft)]'
                            }`}
                          >
                            {isOffline
                              ? t(locale, 'offline')
                              : isSelected
                                ? t(locale, 'active')
                                : `${d.width}×${d.height}`}
                          </span>
                        </div>
                        <div className="flex items-center justify-between gap-2 w-full mt-1">
                          <span className="text-xs text-[var(--ink)] truncate" title={label}>
                            {label}
                          </span>
                          {isSelected && (
                            <span className="text-[11px] font-mono-tabular text-[var(--ink-soft)] shrink-0">
                              {d.width}×{d.height}
                            </span>
                          )}
                        </div>
                      </button>
                    );
                  })}
                </div>
              </div>
            )}

            <div className="session-group">
              <div className="text-xs font-semibold text-[var(--ink-soft)] mb-2">
                {t(locale, 'view')}
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <button
                  type="button"
                  className={`rd-btn${viewMode === 'fit' ? ' primary' : ''}`}
                  data-testid="view-fit"
                  aria-pressed={viewMode === 'fit'}
                  onClick={onFit}
                >
                  {t(locale, 'fit')}
                </button>
                <button
                  type="button"
                  className={`rd-btn${viewMode === 'original' && zoomPercent === 100 ? ' primary' : ''}`}
                  data-testid="view-original"
                  aria-pressed={viewMode === 'original' && zoomPercent === 100}
                  onClick={onOriginal}
                >
                  100%
                </button>
                <button
                  type="button"
                  className="rd-btn font-mono-tabular"
                  data-testid="zoom-out"
                  aria-label={t(locale, 'zoomOut')}
                  data-tooltip={t(locale, 'zoomOut')}
                  data-tooltip-pos="top"
                  onClick={onZoomOut}
                >
                  −
                </button>
                <span
                  className="font-mono-tabular text-xs font-medium text-[var(--ink)] min-w-[46px] text-center"
                  data-testid="zoom"
                >
                  {zoomPercent}%
                </span>
                <button
                  type="button"
                  className="rd-btn font-mono-tabular"
                  data-testid="zoom-in"
                  aria-label={t(locale, 'zoomIn')}
                  data-tooltip={t(locale, 'zoomIn')}
                  data-tooltip-pos="top"
                  onClick={onZoomIn}
                >
                  +
                </button>
                <button
                  type="button"
                  className={`rd-btn${hand ? ' active' : ''}`}
                  data-testid="pan"
                  onClick={onToggleHand}
                >
                  {hand ? t(locale, 'panning') : t(locale, 'pan')}
                </button>
              </div>
            </div>

            <div className="session-group flex flex-wrap items-center gap-2">
              <button
                type="button"
                className="rd-btn"
                data-testid="touch-mode"
                onClick={onToggleTouchMode}
              >
                {touchMode ? t(locale, 'touch') : t(locale, 'mouse')}
              </button>
              <button type="button" className="rd-btn" onClick={onRefresh}>
                <RefreshCw size={14} />
                <span>{t(locale, 'refresh')}</span>
              </button>
              <button
                type="button"
                className={`rd-btn${viewOnly ? ' active' : ''}`}
                data-testid="menu-view-only"
                aria-pressed={viewOnly}
                onClick={onToggleViewOnly}
              >
                {t(locale, 'optViewOnly')}
              </button>
            </div>
          </div>
        )}

        {/* Tab 2: Input (Shortcuts, Soft Keyboard, Right Click, Clipboard) */}
        {activeTab === 'input' && (
          <div className="flex flex-col gap-3.5" role="tabpanel">
            <div className="session-group">
              <div className="text-xs font-semibold text-[var(--ink-soft)] mb-2">
                {t(locale, 'input')}
              </div>
              <div className="flex flex-wrap gap-2" data-testid="quickbar">
                {!mobileLayout && (
                  <>
                    <button type="button" className="rd-btn" onClick={() => onQuickKey('Esc')}>
                      Esc
                    </button>
                    <button type="button" className="rd-btn" onClick={() => onQuickKey('Tab')}>
                      Tab
                    </button>
                  </>
                )}
                <button type="button" className="rd-btn" onClick={() => onQuickKey('Ctrl+C')}>
                  Ctrl+C
                </button>
                <button type="button" className="rd-btn" onClick={() => onQuickKey('Ctrl+V')}>
                  Ctrl+V
                </button>
                <button
                  type="button"
                  className="rd-btn"
                  onClick={() => onQuickKey('Ctrl+Alt+Del')}
                >
                  Ctrl+Alt+Del
                </button>
                <button type="button" className="rd-btn" onClick={() => onQuickKey('Lock')}>
                  {t(locale, 'lockScreen')}
                </button>
                {!mobileLayout && (
                  <>
                    <button
                      type="button"
                      className={`rd-btn${rightArmed ? ' active' : ''}`}
                      onClick={onRightClick}
                    >
                      {t(locale, touchMode ? 'dockArmRight' : 'dockRightAtCursor')}
                    </button>
                    <button
                      type="button"
                      className={`rd-btn${softKeys ? ' active' : ''}`}
                      data-testid="keyboard"
                      onClick={onToggleKeyboard}
                    >
                      {t(locale, 'keyboard')}
                    </button>
                  </>
                )}
              </div>
            </div>

            <div className="session-group">
              <div className="flex items-center justify-between gap-2 mb-2">
                <span className="text-xs font-semibold text-[var(--ink-soft)]">
                  {t(locale, 'clipboard')}
                </span>
                {clipNotice && <span className="text-xs text-[var(--ok)]">{clipNotice}</span>}
              </div>
              <form className="flex gap-2" onSubmit={handleSendClip}>
                <input
                  type="text"
                  className="rd-input flex-1"
                  data-testid="clipboard-input"
                  value={clipDraft}
                  placeholder={t(locale, 'clipboardPlaceholder')}
                  onChange={(e) => setClipDraft(e.target.value)}
                />
                <button
                  type="submit"
                  className="rd-btn primary"
                  data-testid="clipboard-send"
                  disabled={!clipDraft.trim()}
                >
                  {t(locale, 'send')}
                </button>
              </form>
              <PasteKeysButton locale={locale} onPasteKeys={onPasteKeys} pasteDisabled={pasteDisabled} className="rd-btn mt-2" />
              {remoteClipboard && (
                <div className="mt-2 flex items-center justify-between gap-2 rounded-lg bg-[var(--rd-card-bg)] border border-[var(--rd-card-border)] px-2.5 py-1.5">
                  <span
                    className="text-xs text-[var(--ink)] truncate font-mono-tabular"
                    data-testid="clipboard-remote"
                    title={remoteClipboard}
                  >
                    {remoteClipboard}
                  </span>
                  <button type="button" className="rd-btn" onClick={handleCopyRemote}>
                    {t(locale, 'copy')}
                  </button>
                </div>
              )}
            </div>
          </div>
        )}

        {/* Tab 3: Quality & Display Options */}
        {activeTab === 'display' && (
          <div className="flex flex-col gap-3.5" role="tabpanel">
            <div className="session-group">
              <div className="text-xs font-semibold text-[var(--ink-soft)] mb-2">
                {t(locale, 'qualityAndFps')}
              </div>
              <div className="flex flex-wrap items-center gap-2">
                {(
                  [
                    { value: 'low', label: t(locale, 'qualityLow') },
                    { value: 'balanced', label: t(locale, 'qualityBalanced') },
                    { value: 'best', label: t(locale, 'qualityBest') },
                  ] as const
                ).map((q) => (
                  <button
                    key={q.value}
                    type="button"
                    className={`rd-btn${quality === q.value ? ' active' : ''}`}
                    data-testid={`quality-${q.value}`}
                    onClick={() => onChangeQuality(q.value, targetFps)}
                  >
                    {q.label}
                  </button>
                ))}
                <select
                  className="rd-select rd-select-inline font-mono-tabular"
                  aria-label={t(locale, 'targetFps')}
                  data-testid="fps-select"
                  value={targetFps}
                  onChange={(e) => onChangeQuality(quality, Number(e.target.value), preferredCodec)}
                >
                  <option value={15}>15 fps</option>
                  <option value={30}>30 fps</option>
                  <option value={60}>60 fps</option>
                </select>
              </div>
            </div>

            <details className="session-group">
              <summary className="cursor-pointer text-xs font-semibold">{t(locale, 'advancedQuality')}</summary>
              <div className="text-xs font-semibold text-[var(--ink-soft)] my-2">
                {t(locale, 'sessionCodecLabel')}
              </div>
              <div className="flex flex-wrap items-center gap-1.5">
                {(
                  [
                    { value: 'auto', label: t(locale, 'sessionCodecAuto') },
                    { value: 'h264', label: 'H.264' },
                    { value: 'vp8', label: 'VP8' },
                    { value: 'vp9', label: 'VP9' },
                    { value: 'av1', label: 'AV1' },
                  ] as const
                ).map((c) => (
                  <button
                    key={c.value}
                    type="button"
                    className={`rd-btn${preferredCodec === c.value ? ' active' : ''}`}
                    style={{ minHeight: 34, padding: '4px 10px', fontSize: 12 }}
                    data-testid={`session-codec-${c.value}`}
                    onClick={() => onChangeQuality(quality, targetFps, c.value)}
                  >
                    {c.label}
                  </button>
                ))}
              </div>
            </details>

            <div className="session-group flex flex-col gap-1">
              <label className="rd-check-row">
                <input
                  type="checkbox"
                  checked={followRemoteCursor}
                  onChange={onToggleFollowCursor}
                />
                <span>{t(locale, 'followCursor')}</span>
              </label>
              {hasMultipleDisplays && (
                <label className="rd-check-row">
                  <input type="checkbox" checked={followWindow} onChange={onToggleFollowWindow} />
                  <span>{t(locale, 'followWindow')}</span>
                </label>
              )}
              {sizes.length > 0 && (
                <label className="rd-check-row">
                  <span>{t(locale, 'remoteResolution')}</span>
                  <select disabled={viewOnly} value={sizes.findIndex(s => s.width === stats.width && s.height === stats.height)}
                    onChange={e => {
                      const selected = sizes[Number(e.target.value)];
                      if (selected) onResolution(selected.width, selected.height);
                    }}>
                    <option value={-1} disabled>{stats.width} × {stats.height}</option>
                    {sizes.map((size, i) => <option key={i} value={i}>{size.width} × {size.height}</option>)}
                  </select>
                </label>
              )}
              <label className="rd-check-row">
                <input
                  type="checkbox"
                  checked={showRemoteCursor}
                  onChange={onToggleRemoteCursor}
                />
                <span>{t(locale, 'optShowRemoteCursor')}</span>
              </label>
              <label className="rd-check-row">
                <input
                  type="checkbox"
                  checked={enlargeCursor}
                  disabled={!showRemoteCursor || cursorEmbedded}
                  onChange={onToggleEnlargeCursor}
                />
                <span>{t(locale, 'enlargeCursor')}</span>
              </label>
            </div>
          </div>
        )}

        {/* Tab 4: Session overview */}
        {activeTab === 'session' && (
          <div className="flex flex-col gap-3.5" role="tabpanel" data-testid="telemetry">
            <div className="session-group grid grid-cols-3 gap-x-4 gap-y-2.5 text-xs">
              <div>
                <div className="text-[var(--ink-soft)]">{t(locale, 'statCodec')}</div>
                <div className="font-mono-tabular font-semibold text-[var(--ink)] mt-0.5" data-testid="codec">
                  {stats.codec}
                </div>
              </div>
              <div>
                <div className="text-[var(--ink-soft)]">{t(locale, 'statSize')}</div>
                <div
                  className="font-mono-tabular font-semibold text-[var(--ink)] mt-0.5"
                  data-testid="resolution"
                >
                  {stats.width > 0 && stats.height > 0 ? `${stats.width}×${stats.height}` : '—'}
                </div>
              </div>
              <div>
                <div className="text-[var(--ink-soft)]">{t(locale, 'statFps')}</div>
                <div
                  className="font-mono-tabular font-semibold text-[var(--ink)] mt-0.5 flex items-center gap-1.5"
                  data-testid="fps"
                  data-stream-state={stats.streamState}
                  title={
                    stats.streamState === 'idle'
                      ? t(locale, 'statFpsIdleTip')
                      : undefined
                  }
                >
                  <span>{stats.fps}</span>
                  {stats.streamState === 'idle' && (
                    <span className="text-[10px] font-normal text-[var(--ink-soft)]">
                      {t(locale, 'statFpsIdle')}
                    </span>
                  )}
                </div>
              </div>
              <div>
                <div className="text-[var(--ink-soft)]">{t(locale, 'statRtt')}</div>
                <div className="font-mono-tabular font-semibold text-[var(--ink)] mt-0.5" data-testid="delay">
                  {displayRtt} ms
                </div>
              </div>
            </div>

            <div className="session-group">
              <button
                type="button"
                className={`rd-btn${diagnosticsOpen ? ' active' : ''}`}
                onClick={onToggleDiagnostics}
              >
                <Activity size={15} />
                <span>{t(locale, 'diagTitle')}</span>
              </button>
            </div>

            <div className="session-group">
              <label className="rd-check-row">
                <input type="checkbox" checked={audioEnabled} onChange={onToggleAudio} />
                <span>{t(locale, 'remoteAudio')}</span>
              </label>
              {audioIssue && <p role="alert" className="text-xs text-[var(--bad)]">{audioIssue}</p>}
            </div>

            {detail && phase !== 'live' && (
              <p className="m-0 text-xs text-[var(--ink-soft)]">{detail}</p>
            )}
          </div>
        )}

        {decoderError && (
          <div
            className="mt-3 rounded-lg border border-[var(--rd-err-border)] bg-[var(--bad-bg)] px-3 py-2 text-xs text-[var(--bad)] leading-relaxed"
            data-testid="decoder-error"
            role="alert"
          >
            {formatConnectionError(locale, decoderError).message}
          </div>
        )}

        {/* Persistent Footer: Switch Connection + Disconnect */}
        <div className="flex items-center justify-between gap-2 pt-3 mt-3.5 border-t border-[var(--rd-divider)]">
          <button type="button" className="rd-btn" onClick={onOpenConnection}>
            {t(locale, 'connection')}
          </button>
          <button
            type="button"
            className="rd-btn danger"
            data-testid="disconnect"
            onClick={onDisconnect}
          >
            <Power size={15} />
            <span>{t(locale, 'disconnect')}</span>
          </button>
        </div>
      </div>
    </div>
  );
}
