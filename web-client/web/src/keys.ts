const CONTROL_BY_CODE: Record<string, string> = {
  Escape: "Escape",
  Tab: "Tab",
  Backspace: "Backspace",
  Delete: "Delete",
  Enter: "Enter",
  Space: "Space",
  ArrowUp: "ArrowUp",
  ArrowDown: "ArrowDown",
  ArrowLeft: "ArrowLeft",
  ArrowRight: "ArrowRight",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  CapsLock: "CapsLock",
  ControlLeft: "Control",
  ControlRight: "Control",
  AltLeft: "Alt",
  AltRight: "Alt",
  ShiftLeft: "Shift",
  ShiftRight: "Shift",
  MetaLeft: "Meta",
  MetaRight: "Meta",
  F1: "F1",
  F2: "F2",
  F3: "F3",
  F4: "F4",
  F5: "F5",
  F6: "F6",
  F7: "F7",
  F8: "F8",
  F9: "F9",
  F10: "F10",
  F11: "F11",
  F12: "F12",
};

export function controlFromCode(code: string): string | null {
  return CONTROL_BY_CODE[code] ?? null;
}

export function isModifier(name: string): boolean {
  return name === "Control" || name === "Alt" || name === "Shift" || name === "Meta";
}

export interface KeyMods {
  code: string;
  key: string;
  ctrl: boolean;
  alt: boolean;
  meta: boolean;
  shift: boolean;
  repeat: boolean;
}

export function modifierNames(input: Pick<KeyMods, "ctrl" | "alt" | "meta" | "shift">): string[] {
  const names: string[] = [];
  if (input.ctrl) names.push("Control");
  if (input.alt) names.push("Alt");
  if (input.meta) names.push("Meta");
  if (input.shift) names.push("Shift");
  return names;
}

/**
 * These chords close or open a browser tab even when the page calls preventDefault.
 * They are left for the browser and are not sent to the remote desktop.
 */
export function isBrowserReserved(input: Pick<KeyMods, "code" | "ctrl" | "meta">): boolean {
  if (!input.ctrl && !input.meta) return false;
  return input.code === "KeyW" || input.code === "KeyT" || input.code === "KeyN" || input.code === "Tab";
}

export type KeyDownAction =
  | { type: "reserved" }
  | { type: "space-down" }
  | { type: "control"; name: string; modifiers: string[] }
  | { type: "combo"; control?: string; chr?: string; modifiers: string[] }
  | { type: "text"; text: string }
  | { type: "ignore" };

export function keyDownAction(input: KeyMods): KeyDownAction {
  if (isBrowserReserved(input)) return { type: "reserved" };
  const modifiers = modifierNames(input);
  if (input.code === "Space" && !input.ctrl && !input.alt && !input.meta) return { type: "space-down" };
  const control = controlFromCode(input.code);
  if (control && isModifier(control)) return { type: "control", name: control, modifiers };
  if ((input.ctrl || input.alt || input.meta) && !input.repeat) {
    if (control) return { type: "combo", control, modifiers };
    if (input.key.length === 1) return { type: "combo", chr: input.key.toLowerCase(), modifiers };
  }
  if (control) return { type: "control", name: control, modifiers };
  if (input.key.length === 1 && !input.ctrl && !input.alt && !input.meta && !input.repeat) {
    return { type: "text", text: input.key };
  }
  return { type: "ignore" };
}

export type KeyUpAction =
  | { type: "space-up" }
  | { type: "control"; name: string; modifiers: string[] }
  | { type: "combo"; control?: string; chr?: string; modifiers: string[] }
  | { type: "ignore" };

export function keyUpAction(input: KeyMods): KeyUpAction {
  if (isBrowserReserved(input)) return { type: "ignore" };
  const modifiers = modifierNames(input);
  if (input.code === "Space" && !input.ctrl && !input.alt && !input.meta) return { type: "space-up" };
  const control = controlFromCode(input.code);
  if (control && isModifier(control)) return { type: "control", name: control, modifiers };
  if (input.ctrl || input.alt || input.meta) {
    if (control) return { type: "combo", control, modifiers };
    if (input.key.length === 1) return { type: "combo", chr: input.key.toLowerCase(), modifiers };
  }
  if (control) return { type: "control", name: control, modifiers };
  return { type: "ignore" };
}
