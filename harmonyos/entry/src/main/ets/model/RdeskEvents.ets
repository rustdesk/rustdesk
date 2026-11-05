// RustDesk event JSON model + in-process dispatcher.
// Event payloads mirror FlutterHandler::push_event exactly
// (docs/harmonyos-migration/ohos-controller-ffi.md §3): every event carries a
// "name" field; other values are strings, null, or nested JSON strings.

import { evStr, evNum, evBool } from './Utils';

export class RdeskEvent {
  name: string = '';
  values: Record<string, string> = {};
  keys: string[] = [];

  get(key: string): string {
    const v = this.values[key];
    return v !== undefined ? v : '';
  }

  num(key: string): number {
    return evNum(this.get(key));
  }

  bool(key: string): boolean {
    return evBool(this.get(key));
  }
}

export function parseEvent(json: string): RdeskEvent {
  const ev = new RdeskEvent();
  try {
    const obj: Object = JSON.parse(json) as Object;
    const rec: Record<string, Object> = obj as Record<string, Object>;
    const ks: string[] = Object.keys(rec);
    for (const k of ks) {
      const v = rec[k];
      if (k === 'name') {
        ev.name = evStr(v);
      } else if (v !== undefined && v !== null) {
        ev.values[k] = evStr(v);
        ev.keys.push(k);
      }
    }
  } catch (e) {
    ev.name = '';
  }
  return ev;
}

export type EventHandler = (ev: RdeskEvent) => void;
export type ClipboardHandler = (text: string) => void;

class EventDispatcher {
  private handlers: EventHandler[] = [];
  private clipboardHandlers: ClipboardHandler[] = [];

  subscribe(handler: EventHandler): void {
    this.handlers.push(handler);
  }

  unsubscribe(handler: EventHandler): void {
    const i = this.handlers.indexOf(handler);
    if (i >= 0) {
      this.handlers.splice(i, 1);
    }
  }

  dispatch(ev: RdeskEvent): void {
    for (const h of this.handlers) {
      try {
        h(ev);
      } catch (e) {
        console.error(`rustdesk event handler failed: ${e}`);
      }
    }
  }

  subscribeClipboard(handler: ClipboardHandler): void {
    this.clipboardHandlers.push(handler);
  }

  unsubscribeClipboard(handler: ClipboardHandler): void {
    const i = this.clipboardHandlers.indexOf(handler);
    if (i >= 0) {
      this.clipboardHandlers.splice(i, 1);
    }
  }

  dispatchClipboard(text: string): void {
    for (const h of this.clipboardHandlers) {
      try {
        h(text);
      } catch (e) {
        console.error(`rustdesk clipboard handler failed: ${e}`);
      }
    }
  }
}

// Native callbacks are registered once in EntryAbility and fanned out to the
// active page through this singleton.
export const globalEvents: EventDispatcher = new EventDispatcher();
