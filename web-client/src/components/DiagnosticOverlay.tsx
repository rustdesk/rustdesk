import React from 'react';
import { Activity, RotateCcw, X } from 'lucide-react';
import type { DiagnosticsSummary, LinkHealth } from '../lib/diagnostics';
import { t, type Locale } from '../lib/i18n';

interface DiagnosticOverlayProps {
  locale: Locale;
  summary: DiagnosticsSummary;
  hostRttMs: number;
  onReset: () => void;
  onClose: () => void;
}

function formatMs(val: number | null): string {
  if (val === null || !Number.isFinite(val)) return '—';
  return `${val < 10 ? val.toFixed(1) : Math.round(val)} ms`;
}

function formatShortMs(val: number | null): string {
  if (val === null || !Number.isFinite(val)) return '—';
  return val < 10 ? val.toFixed(1) : `${Math.round(val)}`;
}

function healthLabel(locale: Locale, health: LinkHealth): string {
  if (health === 'good') return t(locale, 'diagHealthGood');
  if (health === 'fair') return t(locale, 'diagHealthFair');
  if (health === 'poor') return t(locale, 'diagHealthPoor');
  return t(locale, 'diagHealthWaiting');
}

function healthColor(health: LinkHealth): { dot: string; badge: string } {
  if (health === 'good') {
    return { dot: '#34d399', badge: 'text-[#6ee7b7] bg-[#064e3b]/70 border-[#059669]/50' };
  }
  if (health === 'fair') {
    return { dot: '#fbbf24', badge: 'text-[#fde68a] bg-[#78350f]/70 border-[#d97706]/50' };
  }
  if (health === 'poor') {
    return { dot: '#f87171', badge: 'text-[#fecaca] bg-[#7f1d1d]/80 border-[#ef4444]/60' };
  }
  return { dot: '#94a3b8', badge: 'text-[#cbd5e1] bg-[#1e293b]/80 border-[#475569]/60' };
}

export function DiagnosticOverlay({
  locale,
  summary,
  hostRttMs,
  onReset,
  onClose,
}: DiagnosticOverlayProps) {
  const palette = healthColor(summary.health);
  const lossColor =
    summary.packetLossPct >= 5
      ? 'text-[#f87171]'
      : summary.packetLossPct > 0
        ? 'text-[#fbbf24]'
        : 'text-[#34d399]';

  const maxBarMs = Math.max(
    50,
    ...summary.history.map((h) => (h.rttMs !== null ? h.rttMs : 0)),
  );

  return (
    <aside
      className="diag-overlay"
      data-testid="diagnostic-overlay"
      aria-label={t(locale, 'diagTitle')}
      onPointerDown={(e) => e.stopPropagation()}
      onPointerMove={(e) => e.stopPropagation()}
      onPointerUp={(e) => e.stopPropagation()}
    >
      {/* Header */}
      <div className="flex items-center justify-between gap-2 pb-2 border-b border-white/10">
        <div className="flex items-center gap-1.5 min-w-0">
          <Activity size={14} className="text-[#60a5fa] shrink-0" />
          <span className="text-[12px] font-semibold text-white truncate">
            {t(locale, 'diagTitle')}
          </span>
          <span
            className={`inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-semibold border ${palette.badge}`}
            data-testid="diag-health"
          >
            <span
              className="w-1.5 h-1.5 rounded-full shrink-0"
              style={{ backgroundColor: palette.dot }}
            />
            {healthLabel(locale, summary.health)}
          </span>
        </div>
        <div className="flex items-center gap-1 shrink-0">
          <button
            type="button"
            className="diag-icon-btn"
            data-testid="diag-reset"
            aria-label={t(locale, 'diagReset')}
            title={t(locale, 'diagReset')}
            onClick={onReset}
          >
            <RotateCcw size={13} />
          </button>
          <button
            type="button"
            className="diag-icon-btn"
            data-testid="diag-close"
            aria-label={t(locale, 'diagClose')}
            title={t(locale, 'diagClose')}
            onClick={onClose}
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Primary metrics grid */}
      <div className="grid grid-cols-2 gap-x-3 gap-y-2 pt-2 text-[11px]">
        <div>
          <div className="text-[#94a3b8] text-[10px]">{t(locale, 'diagWsPing')}</div>
          <div
            className="font-mono-tabular text-[14px] font-semibold text-white leading-tight"
            data-testid="diag-ws-rtt"
          >
            {formatMs(summary.currentRttMs)}
          </div>
        </div>

        <div>
          <div className="text-[#94a3b8] text-[10px]">{t(locale, 'diagPacketLoss')}</div>
          <div
            className={`font-mono-tabular text-[14px] font-semibold leading-tight ${lossColor}`}
            data-testid="diag-packet-loss"
          >
            {summary.packetLossPct.toFixed(1)}%
          </div>
        </div>

        <div>
          <div className="text-[#94a3b8] text-[10px]">{t(locale, 'diagJitter')}</div>
          <div
            className="font-mono-tabular text-[12px] font-medium text-[#e2e8f0]"
            data-testid="diag-jitter"
          >
            {formatMs(summary.jitterMs)}
          </div>
        </div>

        <div>
          <div className="text-[#94a3b8] text-[10px]">{t(locale, 'diagHostRtt')}</div>
          <div
            className="font-mono-tabular text-[12px] font-medium text-[#e2e8f0]"
            data-testid="diag-host-rtt"
          >
            {hostRttMs > 0 ? `${Math.round(hostRttMs)} ms` : '—'}
          </div>
        </div>
      </div>

      {/* Rolling latency & loss sparkline */}
      <div
        className="mt-2.5 h-8 rounded-md bg-black/35 border border-white/10 px-1.5 py-1 flex items-end gap-[3px]"
        data-testid="diag-sparkline"
        aria-hidden="true"
      >
        {summary.history.length === 0 ? (
          <div className="w-full h-full flex items-center justify-center text-[10px] text-[#64748b] font-mono-tabular">
            ···
          </div>
        ) : (
          summary.history.map((item) => {
            if (item.lost) {
              return (
                <div
                  key={item.seq}
                  className="flex-1 max-w-[8px] h-full rounded-[2px] bg-[#ef4444]"
                  title={`#${item.seq}: lost`}
                />
              );
            }
            const rtt = item.rttMs ?? 0;
            const pct = Math.max(14, Math.min(100, Math.round((rtt / maxBarMs) * 100)));
            const barColor =
              rtt >= 200 ? 'bg-[#f87171]' : rtt >= 80 ? 'bg-[#fbbf24]' : 'bg-[#34d399]';
            return (
              <div
                key={item.seq}
                className={`flex-1 max-w-[8px] rounded-[2px] ${barColor}`}
                style={{ height: `${pct}%` }}
                title={`#${item.seq}: ${rtt} ms`}
              />
            );
          })
        )}
      </div>

      {/* Footer summary */}
      <div className="mt-2 pt-1.5 border-t border-white/10 flex items-center justify-between gap-2 text-[10px] text-[#94a3b8] font-mono-tabular">
        <span data-testid="diag-minmax" title={t(locale, 'diagMinAvgMax')}>
          {formatShortMs(summary.minRttMs)}/{formatShortMs(summary.avgRttMs)}/
          {formatShortMs(summary.maxRttMs)} ms
        </span>
        <span data-testid="diag-probes">
          {summary.totalReceived}/{summary.totalSent} ({summary.totalLost})
        </span>
      </div>
    </aside>
  );
}
