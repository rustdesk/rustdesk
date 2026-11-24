import React from 'react';
import { Laptop, Monitor, Star, Trash2 } from 'lucide-react';
import { formatPeerId, type SavedDevice } from '../lib/devices';
import { t, type Locale } from '../lib/i18n';

interface PeerListProps {
  locale: Locale;
  devices: SavedDevice[];
  listTab: 'recent' | 'favorites';
  searchQuery: string;
  layout: 'grid' | 'list';
  onSelect: (device: SavedDevice) => void;
  onToggleFavorite: (key: string) => void;
  onRemove: (key: string) => void;
}

function formatPrimaryId(device: SavedDevice): string {
  return device.mode === 'id'
    ? formatPeerId(device.peerId ?? '')
    : `${device.host ?? ''}:${device.port ?? '21118'}`;
}

export function PeerList({
  locale,
  devices,
  listTab,
  searchQuery,
  layout,
  onSelect,
  onToggleFavorite,
  onRemove,
}: PeerListProps) {
  if (devices.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center text-center py-16 px-6">
        <div className="rd-empty-icon">
          {listTab === 'favorites' ? <Star size={22} /> : <Monitor size={22} />}
        </div>
        <p className="m-0 text-sm text-[var(--ink-soft)] max-w-[300px] leading-relaxed">
          {searchQuery.trim()
            ? t(locale, 'noMatchingDevices')
            : listTab === 'favorites'
              ? t(locale, 'noFavorites')
              : t(locale, 'noRecentSessions')}
        </p>
      </div>
    );
  }

  if (layout === 'grid') {
    return (
      <ul
        className="list-none m-0 p-0 grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-3.5"
        data-testid="device-list"
      >
        {devices.map((device) => {
          const primaryId = formatPrimaryId(device);
          const title = device.hostname || primaryId;
          const metaParts = [
            device.platform || null,
            device.mode === 'id' ? t(locale, 'modeId') : t(locale, 'modeDirect'),
          ].filter(Boolean);

          return (
            <li key={device.key} className="rd-device-card">
              <div className="flex items-start gap-2.5">
                <button
                  type="button"
                  className="flex-1 min-w-0 flex items-center gap-3 text-left bg-transparent border-0 p-0"
                  onClick={() => onSelect(device)}
                >
                  <div className="w-10 h-10 rounded-xl bg-[var(--rd-icon-tile-bg)] text-[var(--accent)] flex items-center justify-center shrink-0">
                    {/mac/i.test(device.platform) ? <Laptop size={19} /> : <Monitor size={19} />}
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="text-[14px] font-semibold text-[var(--ink)] truncate">
                      {title}
                    </div>
                    <div className="text-xs font-mono-tabular font-medium text-[var(--rd-id-ink)] truncate mt-0.5">
                      {primaryId}
                    </div>
                  </div>
                </button>

                <div className="flex items-center shrink-0 -mr-1 -mt-1">
                  <button
                    type="button"
                    className="w-8 h-8 inline-flex items-center justify-center rounded-lg border-0 bg-transparent text-[var(--ink-soft)] hover:bg-[var(--rd-hover-bg)] hover:text-[#f59e0b]"
                    aria-label={
                      device.favorite ? t(locale, 'unfavoriteDevice') : t(locale, 'favoriteDevice')
                    }
                    data-tooltip={
                      device.favorite ? t(locale, 'unfavoriteDevice') : t(locale, 'favoriteDevice')
                    }
                    data-tooltip-pos="top"
                    aria-pressed={device.favorite}
                    onClick={() => onToggleFavorite(device.key)}
                  >
                    <Star
                      size={15}
                      className={device.favorite ? 'text-[#f59e0b] fill-[#f59e0b]' : ''}
                    />
                  </button>
                  <button
                    type="button"
                    className="w-8 h-8 inline-flex items-center justify-center rounded-lg border-0 bg-transparent text-[var(--ink-soft)] hover:bg-[var(--bad-bg)] hover:text-[var(--bad)]"
                    aria-label={`${t(locale, 'removeDevice')} ${title}`}
                    data-tooltip={t(locale, 'removeDevice')}
                    data-tooltip-pos="top-end"
                    onClick={() => onRemove(device.key)}
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>

              <button
                type="button"
                className="w-full text-left bg-transparent border-0 p-0 pt-2.5 mt-2.5 border-t border-[var(--rd-divider)] text-xs text-[var(--ink-soft)] truncate"
                onClick={() => onSelect(device)}
              >
                {metaParts.join(' · ')}
              </button>
            </li>
          );
        })}
      </ul>
    );
  }

  return (
    <ul className="list-none m-0 p-0 rd-peer-list-panel" data-testid="device-list">
      {devices.map((device) => {
        const primaryId = formatPrimaryId(device);
        const title = device.hostname || primaryId;
        const metaParts = [
          device.platform || null,
          device.mode === 'id' ? t(locale, 'modeId') : t(locale, 'modeDirect'),
        ].filter(Boolean);

        return (
          <li key={device.key} className="rd-peer-list-row">
            <button
              type="button"
              className="flex-1 min-w-0 flex items-center gap-3 text-left bg-transparent border-0 p-0"
              onClick={() => onSelect(device)}
            >
              <div className="w-9 h-9 rounded-xl bg-[var(--rd-icon-tile-bg)] text-[var(--accent)] flex items-center justify-center shrink-0">
                {/mac/i.test(device.platform) ? <Laptop size={18} /> : <Monitor size={18} />}
              </div>

              <div className="min-w-0 flex-1 sm:grid sm:grid-cols-[minmax(0,1fr)_172px_136px] sm:items-center sm:gap-4">
                <div className="text-[14px] font-semibold text-[var(--ink)] truncate">{title}</div>
                <div className="text-xs font-mono-tabular font-medium text-[var(--rd-id-ink)] truncate mt-0.5 sm:mt-0">
                  {primaryId}
                </div>
                <div className="hidden sm:block text-xs text-[var(--ink-soft)] truncate whitespace-nowrap text-right">
                  {metaParts.join(' · ')}
                </div>
              </div>
            </button>

            <div className="flex items-center gap-0.5 shrink-0">
              <button
                type="button"
                className="w-9 h-9 inline-flex items-center justify-center rounded-lg border-0 bg-transparent text-[var(--ink-soft)] hover:bg-[var(--rd-hover-bg)] hover:text-[#f59e0b]"
                aria-label={
                  device.favorite ? t(locale, 'unfavoriteDevice') : t(locale, 'favoriteDevice')
                }
                data-tooltip={
                  device.favorite ? t(locale, 'unfavoriteDevice') : t(locale, 'favoriteDevice')
                }
                data-tooltip-pos="top"
                aria-pressed={device.favorite}
                onClick={() => onToggleFavorite(device.key)}
              >
                <Star
                  size={16}
                  className={device.favorite ? 'text-[#f59e0b] fill-[#f59e0b]' : ''}
                />
              </button>
              <button
                type="button"
                className="w-9 h-9 inline-flex items-center justify-center rounded-lg border-0 bg-transparent text-[var(--ink-soft)] hover:bg-[var(--bad-bg)] hover:text-[var(--bad)]"
                aria-label={`${t(locale, 'removeDevice')} ${title}`}
                data-tooltip={t(locale, 'removeDevice')}
                data-tooltip-pos="top-end"
                onClick={() => onRemove(device.key)}
              >
                <Trash2 size={15} />
              </button>
            </div>
          </li>
        );
      })}
    </ul>
  );
}
