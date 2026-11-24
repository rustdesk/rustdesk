import React, { useState } from 'react';
import { Activity, Globe, Monitor } from 'lucide-react';
import { formatConnectionError } from '../lib/errors';
import { statusLabel, t, type Locale } from '../lib/i18n';
import type { ViewMode } from '../lib/mapping';
import { formatBitrate, type PeerMessage, type QualityLevel } from '../lib/protocol';
import type { ConnectionPhase } from './RustDeskShell';

export interface SessionStats {
  codec: string;
  width: number;
  height: number;
  fps: number;
  delayMs: number;
  bitrate: number;
  decoded: number;
}

interface SessionMenuProps {
  locale: Locale;
  onToggleLocale: () => void;
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
  isFullscreen: boolean;
  diagnosticsOpen: boolean;
  stats: SessionStats;
  rtt: number;
  quality: QualityLevel;
  targetFps: number;
  remoteClipboard: string;
  onClose: () => void;
  onOpenConnection: () => void;
  onDisconnect: () => void;
  onToggleDiagnostics: () => void;
  onFit: () => void;
  onOriginal: () => void;
  onZoomOut: () => void;
  onZoomIn: () => void;
  onToggleHand: () => void;
  onToggleTouchMode: () => void;
  onSwitchDisplay: (index: number) => void;
  onRefresh: () => void;
  onToggleFullscreen: () => void;
  onQuickKey: (name: 'Esc' | 'Tab' | 'Ctrl+C' | 'Ctrl+V' | 'Ctrl+Alt+Del' | 'Lock') => void;
  onToggleRightArmed: () => void;
  onToggleKeyboard: () => void;
  onChangeQuality: (quality: QualityLevel, fps: number) => void;
  onSendClipboard: (text: string) => void;
}

export function SessionMenu({
  locale,
  onToggleLocale,
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
  isFullscreen,
  diagnosticsOpen,
  stats,
  rtt,
  quality,
  targetFps,
  remoteClipboard,
  onClose,
  onOpenConnection,
  onDisconnect,
  onToggleDiagnostics,
  onFit,
  onOriginal,
  onZoomOut,
  onZoomIn,
  onToggleHand,
  onToggleTouchMode,
  onSwitchDisplay,
  onRefresh,
  onToggleFullscreen,
  onQuickKey,
  onToggleRightArmed,
  onToggleKeyboard,
  onChangeQuality,
  onSendClipboard,
}: SessionMenuProps) {
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
          className="sm:hidden w-10 h-1 rounded-full bg-[#cbd5e1] mx-auto mb-2.5"
          aria-hidden="true"
        />
        {/* Header */}
        <div className="flex items-center justify-between gap-2 pb-3 border-b border-[#e8eef4]">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <h2 className="text-base font-semibold text-[#1c2430] m-0">
                {t(locale, 'session')}
              </h2>
              <span className={`rd-status ${phase}`} data-testid="status">
                {statusLabel(locale, phase)}
              </span>
            </div>
            <p className="text-xs text-[#5c6b7a] m-0 mt-0.5 truncate" data-testid="detail">
              {hostTitle} · {platformLabel} · {linkLabel}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              className="rd-btn"
              data-testid="lang-toggle"
              aria-label={t(locale, 'langToggleAria')}
              onClick={onToggleLocale}
            >
              <Globe size={15} />
              <span>{t(locale, 'langToggle')}</span>
            </button>
            <button
              type="button"
              className="rd-btn"
              data-testid="session-close"
              onClick={onClose}
            >
              {t(locale, 'close')}
            </button>
          </div>
        </div>

        {/* Session actions */}
        <div className="flex flex-wrap gap-2 py-3 border-b border-[#e8eef4]">
          <button type="button" className="rd-btn" onClick={onOpenConnection}>
            {t(locale, 'connection')}
          </button>
          <button
            type="button"
            className={`rd-btn${diagnosticsOpen ? ' active' : ''}`}
            data-testid="toggle-diagnostics"
            aria-pressed={diagnosticsOpen}
            onClick={onToggleDiagnostics}
          >
            <Activity size={15} />
            <span>{t(locale, 'diagnostics')}</span>
          </button>
          <button
            type="button"
            className="rd-btn danger"
            data-testid="disconnect"
            onClick={onDisconnect}
          >
            {t(locale, 'disconnect')}
          </button>
        </div>

        {/* Dedicated Multi-Display Switcher (shown when peer exposes > 1 remote displays) */}
        {hasMultipleDisplays && (
          <div
            className="py-3 border-b border-[#e8eef4]"
            data-testid="display-switcher"
          >
            <div className="flex items-center justify-between gap-2 mb-2">
              <span className="text-xs font-semibold text-[#5c6b7a]">
                {t(locale, 'displays')}
              </span>
              <span className="text-xs font-mono-tabular text-[#5c6b7a]">
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
                      <span className="inline-flex items-center gap-1.5 text-xs font-semibold text-[#1c2430] min-w-0">
                        <Monitor
                          size={15}
                          className={isSelected ? 'text-[#0b6bcb] shrink-0' : 'text-[#5c6b7a] shrink-0'}
                        />
                        <span className="font-mono-tabular">#{d.index + 1}</span>
                      </span>
                      <span
                        className={`text-[11px] font-medium ${
                          isOffline
                            ? 'text-[#b42318]'
                            : isSelected
                              ? 'text-[#0b6bcb]'
                              : 'text-[#5c6b7a]'
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
                      <span className="text-xs text-[#1c2430] truncate" title={label}>
                        {label}
                      </span>
                      {isSelected && (
                        <span className="text-[11px] font-mono-tabular text-[#5c6b7a] shrink-0">
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

        {/* View controls */}
        <div className="py-3 border-b border-[#e8eef4]">
          <div className="text-xs font-semibold text-[#5c6b7a] mb-2">{t(locale, 'view')}</div>
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              className={`rd-btn${viewMode === 'fit' ? ' primary' : ''}`}
              onClick={onFit}
            >
              {t(locale, 'fit')}
            </button>
            <button
              type="button"
              className={`rd-btn${viewMode === 'original' ? ' primary' : ''}`}
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
              className="font-mono-tabular text-xs font-medium text-[#1c2430] min-w-[46px] text-center"
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
            <button
              type="button"
              className="rd-btn"
              data-testid="touch-mode"
              onClick={onToggleTouchMode}
            >
              {touchMode ? t(locale, 'touch') : t(locale, 'mouse')}
            </button>
          </div>

          <div className="flex flex-wrap items-center gap-2 mt-2.5">
            <button type="button" className="rd-btn" onClick={onRefresh}>
              {t(locale, 'refresh')}
            </button>
            <button
              type="button"
              className={`rd-btn${isFullscreen ? ' active' : ''}`}
              data-testid="fullscreen"
              onClick={onToggleFullscreen}
            >
              {isFullscreen ? t(locale, 'exitFullscreen') : t(locale, 'fullscreen')}
            </button>
          </div>
        </div>

        {/* Input quickbar */}
        <div className="py-3 border-b border-[#e8eef4]">
          <div className="text-xs font-semibold text-[#5c6b7a] mb-2">{t(locale, 'input')}</div>
          <div className="flex flex-wrap gap-2" data-testid="quickbar">
            <button type="button" className="rd-btn" onClick={() => onQuickKey('Esc')}>
              Esc
            </button>
            <button type="button" className="rd-btn" onClick={() => onQuickKey('Tab')}>
              Tab
            </button>
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
            <button
              type="button"
              className="rd-btn"
              onClick={() => onQuickKey('Lock')}
            >
              {t(locale, 'lockScreen')}
            </button>
            <button
              type="button"
              className={`rd-btn${rightArmed ? ' active' : ''}`}
              onClick={onToggleRightArmed}
            >
              {rightArmed ? t(locale, 'rightClickArmed') : t(locale, 'rightClick')}
            </button>
            <button
              type="button"
              className={`rd-btn${softKeys ? ' active' : ''}`}
              data-testid="keyboard"
              onClick={onToggleKeyboard}
            >
              {t(locale, 'keyboard')}
            </button>
          </div>
        </div>

        {/* Quality & FPS (M11) */}
        <div className="py-3 border-b border-[#e8eef4]">
          <div className="text-xs font-semibold text-[#5c6b7a] mb-2">
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
              onChange={(e) => onChangeQuality(quality, Number(e.target.value))}
            >
              <option value={15}>15 fps</option>
              <option value={30}>30 fps</option>
              <option value={60}>60 fps</option>
            </select>
          </div>
        </div>

        {/* Clipboard sync (M11) */}
        <div className="py-3 border-b border-[#e8eef4]">
          <div className="flex items-center justify-between gap-2 mb-2">
            <span className="text-xs font-semibold text-[#5c6b7a]">
              {t(locale, 'clipboard')}
            </span>
            {clipNotice && <span className="text-xs text-[#0f7b4c]">{clipNotice}</span>}
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
          {remoteClipboard && (
            <div className="mt-2 flex items-center justify-between gap-2 rounded-lg bg-[#f7f9fb] border border-[#e2e8f0] px-2.5 py-1.5">
              <span
                className="text-xs text-[#1c2430] truncate font-mono-tabular"
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

        {/* Telemetry Stats */}
        <div className="pt-3" data-testid="telemetry">
          <div className="text-xs font-semibold text-[#5c6b7a] mb-2">{t(locale, 'stats')}</div>
          <div className="grid grid-cols-3 gap-x-4 gap-y-2.5 text-xs">
            <div>
              <div className="text-[#5c6b7a]">{t(locale, 'statCodec')}</div>
              <div className="font-mono-tabular font-semibold text-[#1c2430]" data-testid="codec">
                {stats.codec}
              </div>
            </div>
            <div>
              <div className="text-[#5c6b7a]">{t(locale, 'statSize')}</div>
              <div
                className="font-mono-tabular font-semibold text-[#1c2430]"
                data-testid="resolution"
              >
                {stats.width > 0 && stats.height > 0 ? `${stats.width}×${stats.height}` : '—'}
              </div>
            </div>
            <div>
              <div className="text-[#5c6b7a]">{t(locale, 'statFps')}</div>
              <div className="font-mono-tabular font-semibold text-[#1c2430]" data-testid="fps">
                {stats.fps}
              </div>
            </div>
            <div>
              <div className="text-[#5c6b7a]">{t(locale, 'statRtt')}</div>
              <div className="font-mono-tabular font-semibold text-[#1c2430]" data-testid="delay">
                {displayRtt} ms
              </div>
            </div>
            <div>
              <div className="text-[#5c6b7a]">{t(locale, 'statBitrate')}</div>
              <div className="font-mono-tabular font-semibold text-[#1c2430]" data-testid="bitrate">
                {formatBitrate(stats.bitrate)}
              </div>
            </div>
            <div>
              <div className="text-[#5c6b7a]">{t(locale, 'statFrames')}</div>
              <div className="font-mono-tabular font-semibold text-[#1c2430]" data-testid="decoded">
                {stats.decoded}
              </div>
            </div>
          </div>

          {detail && phase !== 'live' && (
            <p className="mt-2 mb-0 text-xs text-[#5c6b7a]">{detail}</p>
          )}

          {decoderError && (
            <div
              className="mt-2.5 rounded-lg border border-[var(--rd-err-border)] bg-[var(--bad-bg)] px-3 py-2 text-xs text-[var(--bad)] leading-relaxed"
              data-testid="decoder-error"
              role="alert"
            >
              {formatConnectionError(locale, decoderError).message}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
