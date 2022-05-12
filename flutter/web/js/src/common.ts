import * as zstd from "zstddec";
import { KeyEvent, controlKeyFromJSON, ControlKey } from "./message";
import { KEY_MAP, LANGS } from "./gen_js_from_hbb";

let decompressor: zstd.ZSTDDecoder;

export async function initZstd() {
  const tmp = new zstd.ZSTDDecoder();
  await tmp.init();
  console.log("zstd ready");
  decompressor = tmp;
}

export async function decompress(compressedArray: Uint8Array) {
  const MAX = 1024 * 1024 * 64;
  const MIN = 1024 * 1024;
  let n = 30 * compressedArray.length;
  if (n > MAX) {
    n = MAX;
  }
  if (n < MIN) {
    n = MIN;
  }
  try {
    if (!decompressor) {
      await initZstd();
    }
    return decompressor.decode(compressedArray, n);
  } catch (e) {
    console.error("decompress failed: " + e);
    return undefined;
  }
}

const LANG = getLang();

export function translate(locale: string, text: string): string {
  const lang = LANG || locale.substring(locale.length - 2).toLowerCase();
  let en = LANGS.en as any;
  let dict = (LANGS as any)[lang];
  if (!dict) dict = en;
  let res = dict[text];
  if (!res && lang != "en") res = en[text];
  return res || text;
}

const zCode = "z".charCodeAt(0);
const aCode = "a".charCodeAt(0);

export function mapKey(name: string, isDesktop: Boolean) {
  const tmp = KEY_MAP[name] || name;
  if (tmp.length == 1) {
    const chr = tmp.charCodeAt(0);
    if (!isDesktop && (chr > zCode || chr < aCode))
      return KeyEvent.fromPartial({ unicode: chr });
    else return KeyEvent.fromPartial({ chr });
  }
  const control_key = controlKeyFromJSON(tmp);
  if (control_key == ControlKey.UNRECOGNIZED) {
    console.error("Unknown control key " + tmp);
  }
  return KeyEvent.fromPartial({ control_key });
}

export async function sleep(ms: number) {
  await new Promise((r) => setTimeout(r, ms));
}

function getLang(): string {
  try {
    const queryString = window.location.search;
    const urlParams = new URLSearchParams(queryString);
    return urlParams.get("lang") || "";
  } catch (e) {
    return "";
  }
}
