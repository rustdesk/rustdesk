import type { CodecSupport, QualityLevel } from './protocol';

export type DefaultViewStyle = 'original' | 'fit';
export type ScrollStyle = 'auto' | 'scrollbar';
export type ImageQualityPreset = 'best' | 'balanced' | 'low' | 'custom';
export type PreferredCodec = 'auto' | 'vp8' | 'vp9' | 'h264' | 'h265' | 'av1';

export interface DisplayPrefs {
  viewStyle: DefaultViewStyle;
  scrollStyle: ScrollStyle;
  qualityPreset: ImageQualityPreset;
  customBitrate: number;
  customFps: number;
  codec: PreferredCodec;
  viewOnly: boolean;
  showRemoteCursor: boolean;
  followWindow: boolean;
  enlargeCursor: boolean;
  autoKeyboard: boolean;
  followRemoteCursor: boolean;
  disableClipboard: boolean;
  lockAfterSessionEnd: boolean;
  reverseMouseWheel: boolean;
  swapMouseButtons: boolean;
}

export const DISPLAY_PREFS_STORAGE_KEY = 'rd_web_display_prefs';

export const DEFAULT_DISPLAY_PREFS: DisplayPrefs = {
  viewStyle: 'fit',
  scrollStyle: 'auto',
  qualityPreset: 'balanced',
  customBitrate: 50,
  customFps: 30,
  codec: 'auto',
  viewOnly: false,
  showRemoteCursor: true,
  followWindow: false,
  enlargeCursor: false,
  autoKeyboard: false,
  followRemoteCursor: false,
  disableClipboard: false,
  lockAfterSessionEnd: false,
  reverseMouseWheel: false,
  swapMouseButtons: false,
};

function clampInt(val: unknown, min: number, max: number, fallback: number): number {
  const n = typeof val === 'number' ? val : Number(val);
  if (!Number.isFinite(n)) return fallback;
  return Math.min(max, Math.max(min, Math.round(n)));
}

export function normalizeDisplayPrefs(raw: unknown): DisplayPrefs {
  if (!raw || typeof raw !== 'object') {
    return { ...DEFAULT_DISPLAY_PREFS };
  }
  const obj = raw as Record<string, unknown>;
  const viewStyle: DefaultViewStyle = obj.viewStyle === 'original' ? 'original' : 'fit';
  const scrollStyle: ScrollStyle = obj.scrollStyle === 'scrollbar' ? 'scrollbar' : 'auto';
  const qualityPreset: ImageQualityPreset =
    obj.qualityPreset === 'best' ||
    obj.qualityPreset === 'balanced' ||
    obj.qualityPreset === 'low' ||
    obj.qualityPreset === 'custom'
      ? obj.qualityPreset
      : DEFAULT_DISPLAY_PREFS.qualityPreset;
  const codec: PreferredCodec =
    obj.codec === 'auto' ||
    obj.codec === 'vp8' ||
    obj.codec === 'vp9' ||
    obj.codec === 'h264' ||
    obj.codec === 'h265' ||
    obj.codec === 'av1'
      ? obj.codec
      : 'auto';

  return {
    viewStyle,
    scrollStyle,
    qualityPreset,
    customBitrate: clampInt(obj.customBitrate, 10, 200, DEFAULT_DISPLAY_PREFS.customBitrate),
    customFps: clampInt(obj.customFps, 10, 60, DEFAULT_DISPLAY_PREFS.customFps),
    codec,
    viewOnly: Boolean(obj.viewOnly),
    followWindow: obj.followWindow === true,
    enlargeCursor: obj.enlargeCursor === true,
    autoKeyboard: obj.autoKeyboard === true,
    followRemoteCursor: obj.followRemoteCursor === true,
    showRemoteCursor:
      typeof obj.showRemoteCursor === 'boolean'
        ? obj.showRemoteCursor
        : DEFAULT_DISPLAY_PREFS.showRemoteCursor,
    disableClipboard: Boolean(obj.disableClipboard),
    lockAfterSessionEnd: Boolean(obj.lockAfterSessionEnd),
    reverseMouseWheel: Boolean(obj.reverseMouseWheel),
    swapMouseButtons: Boolean(obj.swapMouseButtons),
  };
}

export function loadDisplayPrefs(): DisplayPrefs {
  try {
    const raw = window.localStorage.getItem(DISPLAY_PREFS_STORAGE_KEY);
    if (!raw) return { ...DEFAULT_DISPLAY_PREFS };
    return normalizeDisplayPrefs(JSON.parse(raw));
  } catch {
    return { ...DEFAULT_DISPLAY_PREFS };
  }
}

export function persistDisplayPrefs(prefs: DisplayPrefs): void {
  try {
    window.localStorage.setItem(DISPLAY_PREFS_STORAGE_KEY, JSON.stringify(prefs));
  } catch {
    // Ignore storage quota or private-mode errors
  }
}

export function resolveQualityLevel(
  preset: ImageQualityPreset,
  customBitrate: number,
): QualityLevel {
  if (preset === 'best' || preset === 'balanced' || preset === 'low') {
    return preset;
  }
  if (customBitrate >= 75) return 'best';
  if (customBitrate <= 30) return 'low';
  return 'balanced';
}

export function resolveTargetFps(preset: ImageQualityPreset, customFps: number): number {
  if (preset === 'best') return 60;
  if (preset === 'balanced') return 30;
  if (preset === 'low') return 15;
  return clampInt(customFps, 10, 60, 30);
}

export function filterCodecSupport(
  support: CodecSupport,
  preferred: PreferredCodec,
): CodecSupport {
  if (preferred === 'auto' || !support[preferred]) {
    const hasFastCodec = support.h264 || support.vp8 || support.vp9 || support.h265;
    return {
      ...support,
      // Avoid advertising AV1 in auto/fallback mode so hosts without GPU AV1 do not drop to 3-7fps CPU encoding
      av1: hasFastCodec ? false : support.av1,
    };
  }
  return {
    vp8: preferred === 'vp8',
    vp9: preferred === 'vp9',
    h264: preferred === 'h264',
    h265: preferred === 'h265',
    av1: preferred === 'av1',
    reported: support.reported,
  };
}

export function applyMouseButtonSwap(
  button: 'left' | 'right' | 'middle',
  swap: boolean,
): 'left' | 'right' | 'middle' {
  if (!swap) return button;
  if (button === 'left') return 'right';
  if (button === 'right') return 'left';
  return button;
}

export function applyWheelDirection(delta: number, reverse: boolean): number {
  if (delta === 0) return 0;
  return reverse ? -delta : delta;
}
