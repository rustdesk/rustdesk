import React, { useRef } from 'react';
import { t, type Locale } from '../lib/i18n';

interface SessionHandleProps {
  locale: Locale;
  handleX: number;
  stageWidth: number;
  hasWarning: boolean;
  isFullscreen?: boolean;
  onMoveHandle: (nextX: number) => void;
  onOpenMenu: () => void;
}

export function SessionHandle({
  locale,
  handleX,
  stageWidth,
  hasWarning,
  isFullscreen = false,
  onMoveHandle,
  onOpenMenu,
}: SessionHandleProps) {
  const dragRef = useRef<{ startX: number; origin: number; moved: boolean } | null>(null);

  const handlePointerDown = (e: React.PointerEvent<HTMLButtonElement>) => {
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

  const handlePointerMove = (e: React.PointerEvent<HTMLButtonElement>) => {
    const drag = dragRef.current;
    if (!drag) return;
    e.stopPropagation();
    const dx = e.clientX - drag.startX;
    if (Math.abs(dx) > 4) {
      drag.moved = true;
    }
    const width = Math.max(1, stageWidth);
    const next = Math.min(0.92, Math.max(0.08, drag.origin + dx / width));
    onMoveHandle(next);
  };

  const handlePointerUp = (e: React.PointerEvent<HTMLButtonElement>) => {
    const drag = dragRef.current;
    dragRef.current = null;
    e.stopPropagation();
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
    if (drag && !drag.moved) {
      onOpenMenu();
    }
  };

  const handlePointerCancel = (e: React.PointerEvent<HTMLButtonElement>) => {
    dragRef.current = null;
    e.stopPropagation();
  };

  return (
    <button
      type="button"
      className={`pull${hasWarning ? ' warn' : ''}${isFullscreen ? ' fullscreen-subtle' : ''}`}
      data-testid="session-handle"
      aria-label={t(locale, 'openSessionMenu')}
      data-tooltip={t(locale, 'openSessionMenu')}
      data-tooltip-pos="bottom"
      style={{ left: `${handleX * 100}%` }}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerCancel}
    >
      {t(locale, 'session')}
    </button>
  );
}
