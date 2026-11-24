import React, { useRef, useState } from 'react';
import { ChevronLeft, ChevronRight, Keyboard, Layers2, Monitor, MousePointer2 } from 'lucide-react';
import { t, type Locale } from '../lib/i18n';
import type { MobilePanel } from './MobileSessionPanel';

interface Props {
  locale: Locale;
  stageHeight: number;
  onDragTo: (top: number) => void;
  activePanel: MobilePanel | null;
  onToggleKeyboard: () => void;
  onCollapse: () => void;
  onOpenMenu: (panel: MobilePanel) => void;
}

export function MobileDock({ locale, stageHeight, onDragTo, activePanel, onToggleKeyboard, onCollapse, onOpenMenu }: Props) {
  const [hint, setHint] = useState(0);
  const pressStart = useRef<number | null>(null);
  const held = useRef(false);
  const drag = useRef<{ id: number; startX: number; startY: number; y: number } | null>(null);
  const moved = useRef(false);
  const clamp = (v: number, max: number) => Math.max(0, Math.min(max, v));

  const buttons = [
    ['dock-keyboard', 'keyboard', Keyboard, onToggleKeyboard, false],
    ['dock-input-mode', 'pointerSettings', MousePointer2, () => onOpenMenu('input'), activePanel === 'input'],
    ['dock-display', 'screens', Monitor, () => onOpenMenu('display'), activePanel === 'display'],
    ['dock-menu', 'moreActions', Layers2, () => onOpenMenu('actions'), activePanel === 'actions'],
  ] as const;

  if (activePanel) return (
    <nav className="remote-tools" data-testid="mobile-dock" aria-label={t(locale, 'session')} onPointerDown={e => e.stopPropagation()}>
      {buttons.map(([id, label, Icon, action, active]) => (
        <button key={id} type="button" data-testid={id} className={active ? 'selected' : ''} onClick={action}>
          <Icon size={20} /><span>{t(locale, label)}</span>
        </button>
      ))}
      <button type="button" data-testid="dock-collapse" aria-label={t(locale, 'dockHideControls')} onClick={onCollapse}>
        <ChevronRight size={20} />
      </button>
    </nav>
  );

  return (
    <div className="remote-capsule" data-testid="mobile-capsule"
      onPointerDown={e => {
        e.stopPropagation();
        if (!e.isPrimary || e.button !== 0) return;
        const button = (e.target as Element).closest('button');
        if (!button) return;
        button.setPointerCapture(e.pointerId);
        moved.current = false;
        pressStart.current = performance.now();
        held.current = false;
        setHint(0);
        drag.current = { id: e.pointerId, startX: e.clientX, startY: e.clientY, y: e.currentTarget.offsetTop };
      }}
      onPointerMove={e => {
        const d = drag.current;
        if (!d || d.id !== e.pointerId) return;
        e.stopPropagation();
        const dx = e.clientX - d.startX, dy = e.clientY - d.startY;
        if (Math.hypot(dx, dy) > 5) moved.current = true;
        if (moved.current) onDragTo(clamp(d.y + dy, stageHeight - e.currentTarget.offsetHeight));
      }}
      onPointerUp={e => {
        e.stopPropagation();
        if (drag.current?.id !== e.pointerId) return;
        held.current = !moved.current && pressStart.current !== null && performance.now() - pressStart.current >= 450;
        drag.current = null;
      }}
      onContextMenu={e => e.preventDefault()}
      onPointerCancel={() => { drag.current = null; held.current = false; }}
      onClickCapture={e => { if (moved.current && e.detail) { e.stopPropagation(); moved.current = false; } }}>
      <button type="button" data-testid="mobile-dock-expand" aria-label={t(locale, 'openSessionMenu')} onClick={() => onOpenMenu('toolbar')}><ChevronLeft /></button>
      <button type="button" data-testid="capsule-keyboard" aria-label={t(locale, 'holdKeyboard')} title={t(locale, 'holdKeyboard')}
        onClick={e => {
          if (held.current || e.detail === 0) onToggleKeyboard();
          else setHint(n => n + 1);
        }}><Keyboard /></button>
      {hint > 0 && <span key={hint} className="capsule-hint" role="status" onAnimationEnd={() => setHint(0)}>{t(locale, 'holdKeyboard')}</span>}
    </div>
  );
}
