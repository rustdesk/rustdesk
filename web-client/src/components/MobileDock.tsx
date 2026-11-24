import React, { useState } from 'react';
import {
  ChevronDown,
  ChevronUp,
  Hand,
  HelpCircle,
  Keyboard,
  MousePointer2,
  MousePointerClick,
  SlidersHorizontal,
  X,
} from 'lucide-react';
import { t, type Locale } from '../lib/i18n';

interface MobileDockProps {
  locale: Locale;
  touchMode: boolean;
  rightArmed: boolean;
  onToggleKeyboard: () => void;
  onToggleTouchMode: () => void;
  onRightClickAction: () => void;
  onOpenMenu: () => void;
}

export function MobileDock({
  locale,
  touchMode,
  rightArmed,
  onToggleKeyboard,
  onToggleTouchMode,
  onRightClickAction,
  onOpenMenu,
}: MobileDockProps) {
  const [collapsed, setCollapsed] = useState(false);
  const [showGuide, setShowGuide] = useState(false);

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
          <div className="flex items-center justify-between gap-2 pb-2 mb-2 border-b border-[#e8eef4]">
            <span className="text-xs font-semibold text-[#1c2430]">
              {touchMode ? t(locale, 'touchModeGestures') : t(locale, 'mouseModeGestures')}
            </span>
            <button
              type="button"
              className="rd-btn"
              style={{ minWidth: 36, minHeight: 36, padding: 6 }}
              aria-label={t(locale, 'closeGestureGuide')}
              onClick={() => setShowGuide(false)}
            >
              <X size={16} />
            </button>
          </div>
          {touchMode ? (
            <ul className="m-0 p-0 list-none flex flex-col gap-1.5 text-xs text-[#5c6b7a]">
              <li>
                <strong className="text-[#1c2430] font-semibold">
                  {t(locale, 'gTouchTapTitle')}
                </strong>{' '}
                — {t(locale, 'gTouchTapDesc')}
              </li>
              <li>
                <strong className="text-[#1c2430] font-semibold">
                  {t(locale, 'gTouchRightTitle')}
                </strong>{' '}
                — {t(locale, 'gTouchRightDesc')}
              </li>
              <li>
                <strong className="text-[#1c2430] font-semibold">
                  {t(locale, 'gTouchDragTitle')}
                </strong>{' '}
                — {t(locale, 'gTouchDragDesc')}
              </li>
              <li>
                <strong className="text-[#1c2430] font-semibold">
                  {t(locale, 'gTouchPinchTitle')}
                </strong>{' '}
                — {t(locale, 'gTouchPinchDesc')}
              </li>
              <li>
                <strong className="text-[#1c2430] font-semibold">
                  {t(locale, 'gTouchScrollTitle')}
                </strong>{' '}
                — {t(locale, 'gTouchScrollDesc')}
              </li>
            </ul>
          ) : (
            <ul className="m-0 p-0 list-none flex flex-col gap-1.5 text-xs text-[#5c6b7a]">
              <li>
                <strong className="text-[#1c2430] font-semibold">
                  {t(locale, 'gMouseSlideTitle')}
                </strong>{' '}
                — {t(locale, 'gMouseSlideDesc')}
              </li>
              <li>
                <strong className="text-[#1c2430] font-semibold">
                  {t(locale, 'gMouseTapTitle')}
                </strong>{' '}
                — {t(locale, 'gMouseTapDesc')}
              </li>
              <li>
                <strong className="text-[#1c2430] font-semibold">
                  {t(locale, 'gMouseDragTitle')}
                </strong>{' '}
                — {t(locale, 'gMouseDragDesc')}
              </li>
              <li>
                <strong className="text-[#1c2430] font-semibold">
                  {t(locale, 'gMouseRightTitle')}
                </strong>{' '}
                — {t(locale, 'gMouseRightDesc')}
              </li>
              <li>
                <strong className="text-[#1c2430] font-semibold">
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
          className="mobile-dock-fab"
          data-testid="mobile-dock-expand"
          aria-label={t(locale, 'dockShowControls')}
          onPointerDown={(e) => e.stopPropagation()}
          onClick={() => setCollapsed(false)}
        >
          <ChevronUp size={20} />
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
            <span>{t(locale, 'dockMore')}</span>
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
