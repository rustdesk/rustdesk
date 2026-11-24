import React, { useLayoutEffect, useRef, useState } from 'react';
import {
  ChevronDown,
  ChevronUp,
  GripVertical,
  Keyboard,
  Monitor,
  Pin,
  PinOff,
  Power,
  RefreshCw,
  Scan,
  ShieldAlert,
  SlidersHorizontal,
} from 'lucide-react';
import { clampToolbarPixelX, clampToolbarX } from '../lib/floatingMenu';
import { t, type Locale } from '../lib/i18n';
import type { ViewMode } from '../lib/mapping';
import type { DisplayInfo } from '../lib/protocol';

interface SessionHandleProps {
  locale: Locale;
  handleX: number;
  stageWidth: number;
  expanded: boolean;
  pinned: boolean;
  hostTitle: string;
  displays: DisplayInfo[];
  activeDisplay: number;
  viewMode: ViewMode;
  hasWarning: boolean;
  onMoveHandle: (nextX: number) => void;
  onSetExpanded: (expanded: boolean) => void;
  onTogglePinned: () => void;
  onSwitchDisplay: (index: number) => void;
  onToggleViewMode: () => void;
  onRefresh: () => void;
  onToggleKeyboard: () => void;
  onCtrlAltDel: () => void;
  onOpenMenu: () => void;
  onDisconnect: () => void;
}

export function SessionHandle({
  locale,
  handleX,
  stageWidth,
  expanded,
  pinned,
  hostTitle,
  displays,
  activeDisplay,
  viewMode,
  hasWarning,
  onMoveHandle,
  onSetExpanded,
  onTogglePinned,
  onSwitchDisplay,
  onToggleViewMode,
  onRefresh,
  onToggleKeyboard,
  onCtrlAltDel,
  onOpenMenu,
  onDisconnect,
}: SessionHandleProps) {
  const dragRef = useRef<{ startX: number; origin: number; moved: boolean } | null>(null);
  const barRef = useRef<HTMLDivElement>(null);
  const [barWidth, setBarWidth] = useState(520);

  useLayoutEffect(() => {
    if (expanded && barRef.current) {
      setBarWidth(barRef.current.offsetWidth || 520);
    }
  }, [expanded, displays.length, hostTitle, locale]);

  const startDrag = (e: React.PointerEvent<HTMLElement>) => {
    e.stopPropagation();
    try {
      e.currentTarget.setPointerCapture(e.pointerId);
    } catch {
      // Ignore NotFoundError on synthetic pointers
    }
    dragRef.current = {
      startX: e.clientX,
      origin: handleX,
      moved: false,
    };
  };

  const moveDrag = (e: React.PointerEvent<HTMLElement>) => {
    const drag = dragRef.current;
    if (!drag) return;
    e.stopPropagation();
    const dx = e.clientX - drag.startX;
    if (Math.abs(dx) > 4) {
      drag.moved = true;
    }
    const width = Math.max(1, stageWidth);
    onMoveHandle(clampToolbarX(drag.origin + dx / width));
  };

  const endDrag = (e: React.PointerEvent<HTMLElement>, onTap?: () => void) => {
    const drag = dragRef.current;
    dragRef.current = null;
    e.stopPropagation();
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
    if (drag && !drag.moved && onTap) {
      onTap();
    }
  };

  const cancelDrag = (e: React.PointerEvent<HTMLElement>) => {
    dragRef.current = null;
    e.stopPropagation();
  };

  if (!expanded) {
    return (
      <button
        type="button"
        className={`pull${hasWarning ? ' warn' : ''}`}
        data-testid="session-handle"
        aria-label={t(locale, 'toolbarExpand')}
        aria-expanded="false"
        data-tooltip={t(locale, 'toolbarExpand')}
        data-tooltip-pos="bottom"
        style={{ left: `${clampToolbarX(handleX) * 100}%` }}
        onPointerDown={startDrag}
        onPointerMove={moveDrag}
        onPointerUp={(e) => endDrag(e, () => onSetExpanded(true))}
        onPointerCancel={cancelDrag}
      >
        <span className="pull-pill">
          <ChevronDown size={13} strokeWidth={2.2} />
          <span>{t(locale, 'menuLabel')}</span>
        </span>
      </button>
    );
  }

  const leftPx =
    stageWidth > 0 ? clampToolbarPixelX(handleX, barWidth, stageWidth, 12) : undefined;
  const hasMultipleDisplays = displays.length > 1;

  return (
    <div
      ref={barRef}
      className="rd-desktop-toolbar"
      data-testid="desktop-toolbar"
      role="toolbar"
      aria-label={t(locale, 'session')}
      style={leftPx !== undefined ? { left: `${leftPx}px` } : { left: '50%' }}
      onPointerDown={(e) => e.stopPropagation()}
    >
      {/* Drag grip + remote host title */}
      <div
        className="rd-toolbar-drag"
        data-tooltip={t(locale, 'toolbarDragHint')}
        data-tooltip-pos="bottom-start"
        onPointerDown={startDrag}
        onPointerMove={moveDrag}
        onPointerUp={(e) => endDrag(e)}
        onPointerCancel={cancelDrag}
      >
        <GripVertical size={14} className="shrink-0 opacity-60" />
        <span className="rd-toolbar-host">{hostTitle}</span>
      </div>

      <span className="rd-toolbar-sep" aria-hidden="true" />

      {/* Multi-monitor quick switch */}
      {hasMultipleDisplays && (
        <>
          <div className="flex items-center gap-0.5" role="group" aria-label={t(locale, 'displays')}>
            {displays.map((d) => (
              <button
                key={d.index}
                type="button"
                disabled={d.online === false}
                className={`rd-toolbar-btn rd-toolbar-monitor${activeDisplay === d.index ? ' active' : ''}`}
                data-testid={`toolbar-display-${d.index}`}
                aria-pressed={activeDisplay === d.index}
                data-tooltip={`${d.name || `${t(locale, 'displayFallback')} ${d.index + 1}`} (${d.width}×${d.height})`}
                data-tooltip-pos="bottom"
                onClick={() => onSwitchDisplay(d.index)}
              >
                <Monitor size={14} />
                <span className="font-mono-tabular text-[11px]">{d.index + 1}</span>
              </button>
            ))}
          </div>
          <span className="rd-toolbar-sep" aria-hidden="true" />
        </>
      )}

      {/* Frequent 1-click actions */}
      <button
        type="button"
        className={`rd-toolbar-btn${viewMode === 'original' ? ' active' : ''}`}
        data-testid="toolbar-view-toggle"
        aria-label={viewMode === 'fit' ? '100%' : t(locale, 'fit')}
        data-tooltip={viewMode === 'fit' ? '100%' : t(locale, 'fit')}
        data-tooltip-pos="bottom"
        onClick={onToggleViewMode}
      >
        <Scan size={15} />
      </button>

      <button
        type="button"
        className="rd-toolbar-btn"
        data-testid="toolbar-refresh"
        aria-label={t(locale, 'refresh')}
        data-tooltip={t(locale, 'refresh')}
        data-tooltip-pos="bottom"
        onClick={onRefresh}
      >
        <RefreshCw size={15} />
      </button>

      <button
        type="button"
        className="rd-toolbar-btn"
        data-testid="toolbar-cad"
        aria-label="Ctrl+Alt+Del"
        data-tooltip="Ctrl+Alt+Del"
        data-tooltip-pos="bottom"
        onClick={onCtrlAltDel}
      >
        <ShieldAlert size={15} />
      </button>

      <button
        type="button"
        className="rd-toolbar-btn"
        data-testid="toolbar-keyboard"
        aria-label={t(locale, 'keyboard')}
        data-tooltip={t(locale, 'keyboard')}
        data-tooltip-pos="bottom"
        onClick={onToggleKeyboard}
      >
        <Keyboard size={15} />
      </button>

      <span className="rd-toolbar-sep" aria-hidden="true" />

      {/* Open full categorized menu */}
      <button
        type="button"
        className={`rd-toolbar-btn rd-toolbar-menu-btn${hasWarning ? ' warn' : ''}`}
        data-testid="toolbar-open-menu"
        aria-label={t(locale, 'toolbarMore')}
        data-tooltip={t(locale, 'toolbarMore')}
        data-tooltip-pos="bottom"
        onClick={onOpenMenu}
      >
        <SlidersHorizontal size={15} />
        <span className="text-xs font-medium">{t(locale, 'menuLabel')}</span>
      </button>

      {/* Pin / Auto-hide */}
      <button
        type="button"
        className={`rd-toolbar-btn${pinned ? ' active' : ''}`}
        data-testid="toolbar-pin"
        aria-pressed={pinned}
        aria-label={pinned ? t(locale, 'toolbarUnpin') : t(locale, 'toolbarPin')}
        data-tooltip={pinned ? t(locale, 'toolbarUnpin') : t(locale, 'toolbarPin')}
        data-tooltip-pos="bottom"
        onClick={onTogglePinned}
      >
        {pinned ? <Pin size={14} /> : <PinOff size={14} />}
      </button>

      {/* Disconnect */}
      <button
        type="button"
        className="rd-toolbar-btn danger"
        data-testid="toolbar-disconnect"
        aria-label={t(locale, 'disconnect')}
        data-tooltip={t(locale, 'disconnect')}
        data-tooltip-pos="bottom-end"
        onClick={onDisconnect}
      >
        <Power size={15} />
      </button>

      {/* Collapse back to top pull tab */}
      <button
        type="button"
        className="rd-toolbar-btn"
        data-testid="toolbar-collapse"
        aria-label={t(locale, 'toolbarCollapse')}
        data-tooltip={t(locale, 'toolbarCollapse')}
        data-tooltip-pos="bottom-end"
        onClick={() => onSetExpanded(false)}
      >
        <ChevronUp size={15} />
      </button>
    </div>
  );
}
