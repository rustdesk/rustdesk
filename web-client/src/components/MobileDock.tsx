import React, { useRef, useState } from 'react';
import { ChevronLeft, ChevronRight, Keyboard, Layers2, Monitor, MousePointer2 } from 'lucide-react';
import { t, type Locale } from '../lib/i18n';
import type { MobilePanel } from './MobileSessionPanel';

interface Props {
  locale: Locale;
  stageHeight: number;
  activePanel: MobilePanel | null;
  onToggleKeyboard: () => void;
  onCollapse: () => void;
  onOpenMenu: (panel: MobilePanel) => void;
}

export function MobileDock({ locale, stageHeight, activePanel, onToggleKeyboard, onCollapse, onOpenMenu }: Props) {
  const [top, setTop] = useState<number | null>(null);
  const drag = useRef<{ id: number; startX: number; startY: number; y: number } | null>(null);
  const moved = useRef(false);
  const clamp = (v: number, max: number) => Math.max(0, Math.min(max, v));
  const y = clamp(top ?? stageHeight - 84, stageHeight - 44);
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
    <div className="remote-capsule" style={{ top: y }} data-testid="mobile-capsule"
      onPointerDown={e => {
        e.stopPropagation();
        if (!e.isPrimary || e.button !== 0) return;
        const button = (e.target as Element).closest('button');
        if (!button) return;
        button.setPointerCapture(e.pointerId);
        moved.current = false;
        drag.current = { id: e.pointerId, startX: e.clientX, startY: e.clientY, y };
      }}
      onPointerMove={e => {
        const d = drag.current;
        if (!d || d.id !== e.pointerId) return;
        e.stopPropagation();
        const dx = e.clientX - d.startX, dy = e.clientY - d.startY;
        if (Math.hypot(dx, dy) > 5) moved.current = true;
        if (moved.current) setTop(clamp(d.y + dy, stageHeight - 44));
      }}
      onPointerUp={e => { e.stopPropagation(); drag.current = null; }}
      onPointerCancel={() => { drag.current = null; }}
      onClickCapture={e => { if (moved.current && e.detail) { e.stopPropagation(); moved.current = false; } }}>
      <button type="button" data-testid="mobile-dock-expand" aria-label={t(locale, 'openSessionMenu')} onClick={() => onOpenMenu('toolbar')}><ChevronLeft /></button>
      <button type="button" data-testid="capsule-keyboard" aria-label={t(locale, 'keyboard')} onClick={onToggleKeyboard}><Keyboard /></button>
    </div>
  );
}
