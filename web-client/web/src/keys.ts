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
