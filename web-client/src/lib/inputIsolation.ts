export interface KeyboardIsolationContext {
  phase: 'idle' | 'connecting' | 'live' | 'error';
  chromeOpen: boolean;
  insideChromeInput: boolean;
}

/**
 * Determines whether a keyboard event should be fully captured and prevented
 * from triggering browser-level shortcuts (Ctrl+R/F/P/S/W/+/-, Space scroll,
 * Tab focus jump, Backspace navigation, Alt menu activation, F1-F12, etc.).
 */
export function shouldIsolateKeyboardEvent(ctx: KeyboardIsolationContext): boolean {
  if (ctx.phase !== 'live') return false;
  if (ctx.chromeOpen || ctx.insideChromeInput) return false;
  return true;
}

/**
 * Browser mouse buttons whose native default action interferes with remote control:
 * - 1: middle button (autoscroll circle / paste / open-tab)
 * - 3: browser Back button
 * - 4: browser Forward button
 */
export function isBrowserNavOrAuxButton(button: number): boolean {
  return button === 1 || button === 3 || button === 4;
}

/**
 * Determines whether a window-level wheel event (vertical scroll, horizontal
 * back/forward swipe, or Ctrl+wheel pinch-zoom) must be prevented to keep the
 * host browser page stationary during active remote desktop control.
 */
export function shouldPreventWindowWheel(
  remoteInputActive: boolean,
  insideScrollableChrome: boolean,
): boolean {
  if (!remoteInputActive) return false;
  return !insideScrollableChrome;
}

/**
 * Determines whether a touchstart/touchmove event on the stage should prevent
 * browser scrolling, pull-to-refresh, double-tap zoom, and edge-swipe navigation.
 */
export function shouldPreventStageTouch(
  remoteInputActive: boolean,
  isStageSurfaceTarget: boolean,
): boolean {
  return remoteInputActive && isStageSurfaceTarget;
}
