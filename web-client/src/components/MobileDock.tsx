import React, { useRef, useState } from 'react';
import { ChevronLeft, ChevronRight, Keyboard, Layers2, RotateCw, Monitor, MousePointer2 } from 'lucide-react';
import { t, type Locale } from '../lib/i18n';
import type { MobilePanel } from './MobileSessionPanel';

interface Props {
  locale: Locale;
  landscape: boolean;
  onRotate: () => void;
  activePanel: MobilePanel | null;
  onToggleKeyboard: () => void;
  onCollapse: () => void;
  onOpenMenu: (panel: MobilePanel) => void;
}

export function MobileDock({ locale, landscape, onRotate, activePanel, onToggleKeyboard, onCollapse, onOpenMenu }: Props) {
  const [hint, setHint] = useState(0);
  const lastTap = useRef<number | null>(null);

  const buttons = [
    ['dock-keyboard', 'keyboard', Keyboard, onToggleKeyboard, false],
    ['dock-input-mode', 'pointerSettings', MousePointer2, () => onOpenMenu('input'), activePanel === 'input'],
    ['dock-display', 'screens', Monitor, () => onOpenMenu('display'), activePanel === 'display'],
    ['dock-orientation', landscape ? 'portraitLayout' : 'landscapeLayout', RotateCw, onRotate, false],
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
      onPointerDown={e => e.stopPropagation()}
      onContextMenu={e => e.preventDefault()}>
      <button type="button" data-testid="mobile-dock-expand" aria-label={t(locale, 'openSessionMenu')} onClick={() => { lastTap.current = null; setHint(0); onOpenMenu('toolbar'); }}><ChevronLeft /></button>
      <button type="button" data-testid="capsule-keyboard" aria-label={t(locale, 'doubleTapKeyboard')} title={t(locale, 'doubleTapKeyboard')}
        onClick={e => {
          const now = performance.now();
          if (e.detail === 0 || (lastTap.current !== null && now - lastTap.current <= 350)) {
            lastTap.current = null;
            setHint(0);
            onToggleKeyboard();
          } else {
            lastTap.current = now;
            setHint(n => n + 1);
          }
        }}><Keyboard /></button>
      {hint > 0 && <span key={hint} className="capsule-hint" role="status" onAnimationEnd={() => setHint(0)}>{t(locale, 'doubleTapKeyboard')}</span>}
    </div>
  );
}
