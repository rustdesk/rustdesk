import React, { useRef, useState } from 'react';
import {
  ChevronDown,
  Hand,
  HelpCircle,
  Keyboard,
  Menu,
  MousePointer2,
  MousePointerClick,
  SlidersHorizontal,
  X,
} from 'lucide-react';
import {
  clampOrbPosition,
  DEFAULT_ORB_STATE,
  orbCoordsFromState,
  snapOrbToEdge,
  type OrbState,
} from '../lib/floatingMenu';
import { t, type Locale } from '../lib/i18n';

const ORB_SIZE = 46;
const ORB_MARGIN = 10;

interface MobileDockProps {
  locale: Locale;
  stageWidth: number;
  stageHeight: number;
  touchMode: boolean;
  rightArmed: boolean;
  hasWarning: boolean;
  onToggleKeyboard: () => void;
  onToggleTouchMode: () => void;
  onRightClickAction: () => void;
  onOpenMenu: () => void;
}

export function MobileDock({
  locale,
  stageWidth,
  stageHeight,
  touchMode,
  rightArmed,
  hasWarning,
  onToggleKeyboard,
  onToggleTouchMode,
  onRightClickAction,
  onOpenMenu,
}: MobileDockProps) {
  // Start tucked away as a compact draggable floating orb so the remote desktop stays unobstructed
  const [collapsed, setCollapsed] = useState(true);
  const [showGuide, setShowGuide] = useState(false);
  const [orbState, setOrbState] = useState<OrbState>(DEFAULT_ORB_STATE);
  const [dragPos, setDragPos] = useState<{ x: number; y: number } | null>(null);

  const dragRef = useRef<{
    startX: number;
    startY: number;
    origX: number;
    origY: number;
    moved: boolean;
  } | null>(null);

  const vw = stageWidth > 0 ? stageWidth : typeof window !== 'undefined' ? window.innerWidth : 390;
  const vh = stageHeight > 0 ? stageHeight : typeof window !== 'undefined' ? window.innerHeight : 844;
  const currentCoords = dragPos ?? orbCoordsFromState(orbState, ORB_SIZE, vw, vh, ORB_MARGIN);

  const handleOrbPointerDown = (e: React.PointerEvent<HTMLButtonElement>) => {
    e.stopPropagation();
    try {
      e.currentTarget.setPointerCapture(e.pointerId);
    } catch {
      // Ignore synthetic pointer capture error
    }
    const base = orbCoordsFromState(orbState, ORB_SIZE, vw, vh, ORB_MARGIN);
    dragRef.current = {
      startX: e.clientX,
      startY: e.clientY,
      origX: base.x,
      origY: base.y,
      moved: false,
    };
  };

  const handleOrbPointerMove = (e: React.PointerEvent<HTMLButtonElement>) => {
    const drag = dragRef.current;
    if (!drag) return;
    e.stopPropagation();
    const dx = e.clientX - drag.startX;
    const dy = e.clientY - drag.startY;
    if (Math.hypot(dx, dy) > 6) {
      drag.moved = true;
    }
    if (drag.moved) {
      setDragPos(clampOrbPosition(drag.origX + dx, drag.origY + dy, ORB_SIZE, vw, vh, ORB_MARGIN));
    }
  };

  const handleOrbPointerUp = (e: React.PointerEvent<HTMLButtonElement>) => {
    const drag = dragRef.current;
    dragRef.current = null;
    e.stopPropagation();
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
    if (!drag) return;
    if (drag.moved) {
      const dx = e.clientX - drag.startX;
      const dy = e.clientY - drag.startY;
      setOrbState(snapOrbToEdge(drag.origX + dx, drag.origY + dy, ORB_SIZE, vw, vh, ORB_MARGIN));
      setDragPos(null);
    } else {
      setDragPos(null);
      setCollapsed(false);
    }
  };

  const handleOrbPointerCancel = (e: React.PointerEvent<HTMLButtonElement>) => {
    dragRef.current = null;
    setDragPos(null);
    e.stopPropagation();
  };

  return (
    <>
      {showGuide && (
        <div
          className="gesture-guide"
          data-testid="gesture-guide"
          role="dialog"
          aria-label={
            touchMode ? t(locale, 'touchModeGestures') : t(locale, 'mouseModeGestures')
          }
          onPointerDown={(e) => e.stopPropagation()}
        >
          <div className="flex items-center justify-between gap-2 pb-2 mb-2 border-b border-[var(--rd-divider)]">
            <span className="text-xs font-semibold text-[var(--ink)]">
              {touchMode ? t(locale, 'touchModeGestures') : t(locale, 'mouseModeGestures')}
            </span>
            <button
              type="button"
              className="rd-btn"
              style={{ minWidth: 34, minHeight: 34, padding: 6 }}
              aria-label={t(locale, 'closeGestureGuide')}
              onClick={() => setShowGuide(false)}
            >
              <X size={16} />
            </button>
          </div>
          {touchMode ? (
            <ul className="m-0 p-0 list-none flex flex-col gap-1.5 text-xs text-[var(--ink-soft)]">
              <li>
                <strong className="text-[var(--ink)] font-semibold">
                  {t(locale, 'gTouchTapTitle')}
                </strong>{' '}
                — {t(locale, 'gTouchTapDesc')}
              </li>
              <li>
                <strong className="text-[var(--ink)] font-semibold">
                  {t(locale, 'gTouchRightTitle')}
                </strong>{' '}
                — {t(locale, 'gTouchRightDesc')}
              </li>
              <li>
                <strong className="text-[var(--ink)] font-semibold">
                  {t(locale, 'gTouchDragTitle')}
                </strong>{' '}
                — {t(locale, 'gTouchDragDesc')}
              </li>
              <li>
                <strong className="text-[var(--ink)] font-semibold">
                  {t(locale, 'gTouchPinchTitle')}
                </strong>{' '}
                — {t(locale, 'gTouchPinchDesc')}
              </li>
              <li>
                <strong className="text-[var(--ink)] font-semibold">
                  {t(locale, 'gTouchScrollTitle')}
                </strong>{' '}
                — {t(locale, 'gTouchScrollDesc')}
              </li>
            </ul>
          ) : (
            <ul className="m-0 p-0 list-none flex flex-col gap-1.5 text-xs text-[var(--ink-soft)]">
              <li>
                <strong className="text-[var(--ink)] font-semibold">
                  {t(locale, 'gMouseSlideTitle')}
                </strong>{' '}
                — {t(locale, 'gMouseSlideDesc')}
              </li>
              <li>
                <strong className="text-[var(--ink)] font-semibold">
                  {t(locale, 'gMouseTapTitle')}
                </strong>{' '}
                — {t(locale, 'gMouseTapDesc')}
              </li>
              <li>
                <strong className="text-[var(--ink)] font-semibold">
                  {t(locale, 'gMouseDragTitle')}
                </strong>{' '}
                — {t(locale, 'gMouseDragDesc')}
              </li>
              <li>
                <strong className="text-[var(--ink)] font-semibold">
                  {t(locale, 'gMouseRightTitle')}
                </strong>{' '}
                — {t(locale, 'gMouseRightDesc')}
              </li>
              <li>
                <strong className="text-[var(--ink)] font-semibold">
                  {t(locale, 'gMousePinchTitle')}
                </strong>{' '}
                — {t(locale, 'gMousePinchDesc')}
              </li>
            </ul>
          )}
        </div>
      )}

      {collapsed ? (
        <button
          type="button"
          className={`mobile-dock-orb${dragPos ? ' dragging' : ''}${hasWarning ? ' warn' : ''}`}
          data-testid="mobile-dock-expand"
          aria-label={t(locale, 'mobileOrbAria')}
          aria-expanded="false"
          style={{
            left: `${currentCoords.x}px`,
            top: `${currentCoords.y}px`,
          }}
          onPointerDown={handleOrbPointerDown}
          onPointerMove={handleOrbPointerMove}
          onPointerUp={handleOrbPointerUp}
          onPointerCancel={handleOrbPointerCancel}
        >
          <Menu size={20} />
        </button>
      ) : (
        <nav
          className="mobile-dock"
          data-testid="mobile-dock"
          aria-label={t(locale, 'session')}
          onPointerDown={(e) => e.stopPropagation()}
        >
          <button
            type="button"
            className="mobile-dock-btn"
            data-testid="dock-keyboard"
            aria-label={t(locale, 'dockOpenKeyboard')}
            onClick={() => {
              setShowGuide(false);
              onToggleKeyboard();
            }}
          >
            <Keyboard size={18} />
            <span>{t(locale, 'dockKeys')}</span>
          </button>

          <button
            type="button"
            className={`mobile-dock-btn${!touchMode ? ' active' : ''}`}
            data-testid="dock-input-mode"
            aria-label={
              touchMode ? t(locale, 'dockSwitchToMouse') : t(locale, 'dockSwitchToTouch')
            }
            onClick={onToggleTouchMode}
          >
            {touchMode ? <Hand size={18} /> : <MousePointer2 size={18} />}
            <span>{touchMode ? t(locale, 'touch') : t(locale, 'mouse')}</span>
          </button>

          <button
            type="button"
            className={`mobile-dock-btn${rightArmed ? ' active' : ''}`}
            data-testid="dock-right-click"
            aria-label={
              touchMode ? t(locale, 'dockArmRight') : t(locale, 'dockRightAtCursor')
            }
            onClick={onRightClickAction}
          >
            <MousePointerClick size={18} />
            <span>{rightArmed ? t(locale, 'dockArmed') : t(locale, 'dockRight')}</span>
          </button>

          <button
            type="button"
            className={`mobile-dock-btn${showGuide ? ' active' : ''}`}
            data-testid="dock-gestures"
            aria-label={t(locale, 'dockGestureGuide')}
            onClick={() => setShowGuide((g) => !g)}
          >
            <HelpCircle size={18} />
            <span>{t(locale, 'dockGuide')}</span>
          </button>

          <button
            type="button"
            className="mobile-dock-btn"
            data-testid="dock-menu"
            aria-label={t(locale, 'openSessionMenu')}
            onClick={() => {
              setShowGuide(false);
              onOpenMenu();
            }}
          >
            <SlidersHorizontal size={18} />
            <span>{t(locale, 'menuLabel')}</span>
          </button>

          <button
            type="button"
            className="mobile-dock-btn mobile-dock-collapse"
            data-testid="mobile-dock-collapse"
            aria-label={t(locale, 'dockHideControls')}
            onClick={() => {
              setShowGuide(false);
              setCollapsed(true);
            }}
          >
            <ChevronDown size={18} />
          </button>
        </nav>
      )}
    </>
  );
}
