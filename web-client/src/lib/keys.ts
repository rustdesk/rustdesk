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

const MODIFIERS = new Set(['Control', 'Alt', 'Shift', 'Meta']);

export function controlFromCode(code: string): string | null {
  return CODE_TO_CONTROL[code] ?? null;
}

export function isModifier(name: string): boolean {
  return MODIFIERS.has(name);
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
 * Appendix B fix #4: forwards Space, control keys, and Ctrl/Alt/Meta + letter chords
 * cleanly with modifiers to the remote host instead of dropping shortcuts or hijacking Space.
 */
export function mapPhysicalKeyDown(ev: KeyboardEventLike): MappedKeyMessage | null {
  const control = controlFromCode(ev.code);
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
    if (!ev.repeat) {
      return { type: 'text', text: ev.key };
    }
  }

  return null;
}

export function mapPhysicalKeyUp(ev: KeyboardEventLike): MappedKeyMessage | null {
  const control = controlFromCode(ev.code);
  const modifiers = activeModifiers(ev);

  if (control) {
    return {
      type: 'key',
      action: 'up',
      control,
      modifiers: modifiers.length > 0 ? modifiers : undefined,
    };
  }

  if (ev.key.length === 1 && (ev.ctrlKey || ev.altKey || ev.metaKey)) {
    return {
      type: 'key',
      action: 'up',
      chr: ev.key.toLowerCase(),
      modifiers,
    };
  }

  return null;
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

