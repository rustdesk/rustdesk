// ArkTS wrapper around the native module (libentry.so -> librustdesk.so).
// Native calls throw JS Errors when librustdesk.so is missing or a contract
// symbol cannot be resolved; this wrapper logs and degrades to safe return
// values so the UI never crashes on a broken install.

import native from 'libentry.so';
import { hilog } from '@kit.PerformanceAnalysisKit';

const LOG_DOMAIN = 0x5253;
const TAG = 'RustDeskFfi';

function logFail(what: string, e: Object): void {
  hilog.error(LOG_DOMAIN, TAG, '%{public}s failed: %{public}s', what, `${e}`);
}

export function ffiInit(home: string, appDir: string, customConfig: string): number {
  try {
    return native.init(home, appDir, customConfig);
  } catch (e) {
    logFail('init', e as Object);
    return -1;
  }
}

export function ffiSetCallbacks(onEvent: (json: string) => void,
                                onClipboard: (text: string) => void): number {
  try {
    return native.setCallbacks(onEvent, onClipboard);
  } catch (e) {
    logFail('setCallbacks', e as Object);
    return -1;
  }
}

export function ffiInitVideo(surfaceId: string): number {
  try {
    return native.initVideo(surfaceId);
  } catch (e) {
    logFail('initVideo', e as Object);
    return -1;
  }
}

export function ffiSessionNew(peerId: string, isFileTransfer: boolean, password: string,
                              forceRelay: boolean): string {
  try {
    return native.sessionNew(peerId, isFileTransfer, password, forceRelay);
  } catch (e) {
    logFail('sessionNew', e as Object);
    return '';
  }
}

export function ffiSessionStart(uuid: string): number {
  try {
    return native.sessionStart(uuid);
  } catch (e) {
    logFail('sessionStart', e as Object);
    return -1;
  }
}

export function ffiSessionLogin(uuid: string, password: string, remember: boolean): number {
  try {
    return native.sessionLogin(uuid, password, remember);
  } catch (e) {
    logFail('sessionLogin', e as Object);
    return -1;
  }
}

export function ffiSessionSend2fa(uuid: string, code: string, trustThisDevice: boolean): number {
  try {
    return native.sessionSend2fa(uuid, code, trustThisDevice);
  } catch (e) {
    logFail('sessionSend2fa', e as Object);
    return -1;
  }
}

export function ffiSessionReconnect(uuid: string, forceRelay: boolean): number {
  try {
    return native.sessionReconnect(uuid, forceRelay);
  } catch (e) {
    logFail('sessionReconnect', e as Object);
    return -1;
  }
}

export function ffiSessionClose(uuid: string): number {
  try {
    return native.sessionClose(uuid);
  } catch (e) {
    logFail('sessionClose', e as Object);
    return -1;
  }
}

export function ffiSendMouse(uuid: string, mask: number, x: number, y: number, alt: boolean,
                             ctrl: boolean, shift: boolean, command: boolean): number {
  try {
    return native.sendMouse(uuid, mask, x, y, alt, ctrl, shift, command);
  } catch (e) {
    logFail('sendMouse', e as Object);
    return -1;
  }
}

export function ffiInputKey(uuid: string, name: string, down: boolean, press: boolean,
                            alt: boolean, ctrl: boolean, shift: boolean, command: boolean): number {
  try {
    return native.inputKey(uuid, name, down, press, alt, ctrl, shift, command);
  } catch (e) {
    logFail('inputKey', e as Object);
    return -1;
  }
}

export function ffiInputString(uuid: string, value: string): number {
  try {
    return native.inputString(uuid, value);
  } catch (e) {
    logFail('inputString', e as Object);
    return -1;
  }
}

export function ffiRefreshVideo(uuid: string): number {
  try {
    return native.refreshVideo(uuid);
  } catch (e) {
    logFail('refreshVideo', e as Object);
    return -1;
  }
}

export function ffiSwitchDisplay(uuid: string, display: number): number {
  try {
    return native.switchDisplay(uuid, display);
  } catch (e) {
    logFail('switchDisplay', e as Object);
    return -1;
  }
}

export function ffiSendClipboard(uuid: string, text: string): number {
  try {
    return native.sendClipboard(uuid, text);
  } catch (e) {
    logFail('sendClipboard', e as Object);
    return -1;
  }
}

export function ffiSendFiles(uuid: string, actId: number, path: string, to: string,
                             fileNum: number, includeHidden: boolean, isRemote: boolean): number {
  try {
    return native.sendFiles(uuid, actId, path, to, fileNum, includeHidden, isRemote);
  } catch (e) {
    logFail('sendFiles', e as Object);
    return -1;
  }
}

export function ffiSetConfirmOverrideFile(uuid: string, actId: number, fileNum: number,
                                          needOverride: boolean, remember: boolean,
                                          isUpload: boolean): number {
  try {
    return native.setConfirmOverrideFile(uuid, actId, fileNum, needOverride, remember, isUpload);
  } catch (e) {
    logFail('setConfirmOverrideFile', e as Object);
    return -1;
  }
}

export function ffiCancelJob(uuid: string, actId: number): number {
  try {
    return native.cancelJob(uuid, actId);
  } catch (e) {
    logFail('cancelJob', e as Object);
    return -1;
  }
}

export function ffiRemoveFile(uuid: string, actId: number, path: string, fileNum: number,
                              isRemote: boolean): number {
  try {
    return native.removeFile(uuid, actId, path, fileNum, isRemote);
  } catch (e) {
    logFail('removeFile', e as Object);
    return -1;
  }
}

export function ffiReadDir(uuid: string, path: string, includeHidden: boolean,
                           isRemote: boolean): number {
  try {
    return native.readDir(uuid, path, includeHidden, isRemote);
  } catch (e) {
    logFail('readDir', e as Object);
    return -1;
  }
}

export function ffiVersion(): string {
  try {
    return native.version();
  } catch (e) {
    logFail('version', e as Object);
    return '';
  }
}

export function ffiGetOption(key: string): string {
  try {
    return native.getOption(key);
  } catch (e) {
    logFail('getOption', e as Object);
    return '';
  }
}

export function ffiSetOption(key: string, value: string): number {
  try {
    return native.setOption(key, value);
  } catch (e) {
    logFail('setOption', e as Object);
    return -1;
  }
}
