const CODE_TO_CONTROL: Record<string, string> = {
  Escape: 'Escape',
  Tab: 'Tab',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Enter: 'Enter',
  NumpadEnter: 'Enter',
  Space: 'Space',
  ArrowUp: 'ArrowUp',
  ArrowDown: 'ArrowDown',
  ArrowLeft: 'ArrowLeft',
  ArrowRight: 'ArrowRight',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
  CapsLock: 'CapsLock',
  ControlLeft: 'Control',
  ControlRight: 'Control',
  AltLeft: 'Alt',
  AltRight: 'Alt',
  ShiftLeft: 'Shift',
  ShiftRight: 'Shift',
  MetaLeft: 'Meta',
  MetaRight: 'Meta',
  F1: 'F1',
  F2: 'F2',
  F3: 'F3',
  F4: 'F4',
  F5: 'F5',
  F6: 'F6',
  F7: 'F7',
  F8: 'F8',
  F9: 'F9',
  F10: 'F10',
  F11: 'F11',
  F12: 'F12',
};

const KEY_TO_CONTROL: Record<string, string> = {
  Escape: 'Escape',
  Esc: 'Escape',
  Tab: 'Tab',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Del: 'Delete',
  Enter: 'Enter',
  ArrowUp: 'ArrowUp',
  ArrowDown: 'ArrowDown',
  ArrowLeft: 'ArrowLeft',
  ArrowRight: 'ArrowRight',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
  CapsLock: 'CapsLock',
  Control: 'Control',
  Alt: 'Alt',
  Shift: 'Shift',
  Meta: 'Meta',
  F1: 'F1',
  F2: 'F2',
  F3: 'F3',
  F4: 'F4',
  F5: 'F5',
  F6: 'F6',
  F7: 'F7',
  F8: 'F8',
  F9: 'F9',
  F10: 'F10',
  F11: 'F11',
  F12: 'F12',
};

const MODIFIERS = new Set(['Control', 'Alt', 'Shift', 'Meta']);

export function controlFromCode(code: string): string | null {
  return CODE_TO_CONTROL[code] ?? null;
}

/**
 * Resolves a RustDesk control key name from physical `code` first, falling back to `key`
 * (important for mobile soft keyboards where `event.code` is often empty or 'Unidentified').
 */
export function controlFromKeyOrCode(code: string, key?: string): string | null {
  const byCode = CODE_TO_CONTROL[code];
  if (byCode) return byCode;
  if (key) {
    return KEY_TO_CONTROL[key] ?? null;
  }
  return null;
}

export function isModifier(name: string): boolean {
  return MODIFIERS.has(name);
}

/**
 * Detects browser IME composition events (Chinese/Japanese/Korean input methods, dead keys)
 * so physical key listeners let the native IME compose cleanly without dropping or intercepting keys.
 */
export function isImeEvent(ev: {
  isComposing?: boolean;
  key?: string;
  keyCode?: number;
}): boolean {
  return Boolean(
    ev.isComposing || ev.key === 'Process' || ev.key === 'Dead' || ev.keyCode === 229,
  );
}

export function isPrintableAscii(ch: string): boolean {
  if (ch.length !== 1) return false;
  const cp = ch.charCodeAt(0);
  return cp >= 0x21 && cp <= 0x7e;
}

export function activeModifiers(ev: {
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}): string[] {
  const mods: string[] = [];
  if (ev.ctrlKey) mods.push('Control');
  if (ev.altKey) mods.push('Alt');
  if (ev.shiftKey) mods.push('Shift');
  if (ev.metaKey) mods.push('Meta');
  return mods;
}

export interface KeyboardEventLike {
  code: string;
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
  repeat: boolean;
  isComposing?: boolean;
  keyCode?: number;
}

export type MappedKeyMessage =
  | {
      type: 'key';
      action: 'down' | 'up';
      control?: string;
      chr?: string;
      modifiers?: string[];
    }
  | { type: 'text'; text: string };

/**
 * Maps physical keydown events to native RustDesk Legacy key events:
 * - IME composition events return null so the hidden capture input handles CJK commits cleanly.
 * - Control keys & Space send `control` down.
 * - Printable ASCII keys send real `chr` down (supports key repeat, terminals, VMs, and Shift cleanly).
 * - Non-ASCII direct characters (e.g., direct Unicode punctuation) send `text`.
 */
export function mapPhysicalKeyDown(ev: KeyboardEventLike): MappedKeyMessage | null {
  if (isImeEvent(ev)) {
    return null;
  }

  const control = controlFromKeyOrCode(ev.code, ev.key);
  const modifiers = activeModifiers(ev);

  if (control) {
    return {
      type: 'key',
      action: 'down',
      control,
      modifiers: modifiers.length > 0 ? modifiers : undefined,
    };
  }

  if (ev.key.length === 1) {
    if (ev.ctrlKey || ev.altKey || ev.metaKey) {
      return {
        type: 'key',
        action: 'down',
        chr: ev.key.toLowerCase(),
        modifiers,
      };
    }
    if (isPrintableAscii(ev.key)) {
      return {
        type: 'key',
        action: 'down',
        chr: ev.key,
        modifiers: modifiers.length > 0 ? modifiers : undefined,
      };
    }
    return { type: 'text', text: ev.key };
  }

  return null;
}

/**
 * Maps physical keyup events, using an optional `trackedChr` so releasing Shift before a letter
 * still releases the exact character that was pressed on keydown.
 */
export function mapPhysicalKeyUp(
  ev: KeyboardEventLike,
  trackedChr?: string,
): MappedKeyMessage | null {
  if (isImeEvent(ev)) {
    return null;
  }

  const control = controlFromKeyOrCode(ev.code, ev.key);
  const modifiers = activeModifiers(ev);

  if (control) {
    return {
      type: 'key',
      action: 'up',
      control,
      modifiers: modifiers.length > 0 ? modifiers : undefined,
    };
  }

  if (trackedChr) {
    return {
      type: 'key',
      action: 'up',
      chr: trackedChr,
      modifiers: modifiers.length > 0 ? modifiers : undefined,
    };
  }

  if (ev.key.length === 1) {
    if (ev.ctrlKey || ev.altKey || ev.metaKey) {
      return {
        type: 'key',
        action: 'up',
        chr: ev.key.toLowerCase(),
        modifiers,
      };
    }
    if (isPrintableAscii(ev.key)) {
      return {
        type: 'key',
        action: 'up',
        chr: ev.key,
        modifiers: modifiers.length > 0 ? modifiers : undefined,
      };
    }
  }

  return null;
}

export interface TextInputDiff {
  backspaces: number;
  insert: string;
}

/**
 * Computes the minimal Backspace count + inserted text delta between two Unicode strings
 * (operating on full code points so CJK characters and emoji are never split mid-surrogate).
 */
export function diffTextInput(prev: string, next: string): TextInputDiff {
  if (prev === next) {
    return { backspaces: 0, insert: '' };
  }
  const prevChars = Array.from(prev);
  const nextChars = Array.from(next);
  const maxCommon = Math.min(prevChars.length, nextChars.length);
  let common = 0;
  while (common < maxCommon && prevChars[common] === nextChars[common]) {
    common += 1;
  }
  return {
    backspaces: prevChars.length - common,
    insert: nextChars.slice(common).join(''),
  };
}

export interface SoftChordPart {
  control?: string;
  chr?: string;
}

/**
 * Builds an ordered key-chord sequence from latched mobile sticky modifiers
 * (e.g. ["Control"]) plus a target control key or character.
 */
export function buildSoftKeyChord(
  latchedModifiers: readonly string[],
  target: SoftChordPart,
): SoftChordPart[] {
  const mods = latchedModifiers.filter((m) => isModifier(m));
  return [...mods.map((m) => ({ control: m })), target];
}
