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

// Session kind for ffiSessionNew (contract §2): remote control, file
// transfer, view camera.
export const SESSION_KIND_CONTROL = 0;
export const SESSION_KIND_FILE_TRANSFER = 1;
export const SESSION_KIND_VIEW_CAMERA = 2;

export function ffiSessionNew(peerId: string, sessionKind: number, password: string,
                              forceRelay: boolean): string {
  try {
    return native.sessionNew(peerId, sessionKind, password, forceRelay);
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

export function ffiRecordScreen(uuid: string, start: boolean): number {
  try {
    return native.recordScreen(uuid, start);
  } catch (e) {
    logFail('recordScreen', e as Object);
    return -1;
  }
}

export function ffiRequestVoiceCall(uuid: string): number {
  try {
    return native.requestVoiceCall(uuid);
  } catch (e) {
    logFail('requestVoiceCall', e as Object);
    return -1;
  }
}

export function ffiCloseVoiceCall(uuid: string): number {
  try {
    return native.closeVoiceCall(uuid);
  } catch (e) {
    logFail('closeVoiceCall', e as Object);
    return -1;
  }
}

export function ffiAnswerVoiceCall(uuid: string, accept: boolean): number {
  try {
    return native.answerVoiceCall(uuid, accept);
  } catch (e) {
    logFail('answerVoiceCall', e as Object);
    return -1;
  }
}

export function ffiSendClipboardImagePng(uuid: string, pngBase64: string): number {
  try {
    return native.sendClipboardImagePng(uuid, pngBase64);
  } catch (e) {
    logFail('sendClipboardImagePng', e as Object);
    return -1;
  }
}

// ---------------------------------------------------------------------------
// Display options: image quality / codec preference / view-only (contract §2).
// ---------------------------------------------------------------------------

export function ffiSessionGetOption(uuid: string, key: string): string {
  try {
    return native.sessionGetOption(uuid, key);
  } catch (e) {
    logFail('sessionGetOption', e as Object);
    return '';
  }
}

export function ffiSessionSetOption(uuid: string, key: string, value: string): number {
  try {
    return native.sessionSetOption(uuid, key, value);
  } catch (e) {
    logFail('sessionSetOption', e as Object);
    return -1;
  }
}

export function ffiSaveImageQuality(uuid: string, value: string): number {
  try {
    return native.saveImageQuality(uuid, value);
  } catch (e) {
    logFail('saveImageQuality', e as Object);
    return -1;
  }
}

export function ffiSaveCustomImageQuality(uuid: string, quality: number): number {
  try {
    return native.saveCustomImageQuality(uuid, quality);
  } catch (e) {
    logFail('saveCustomImageQuality', e as Object);
    return -1;
  }
}

export function ffiSetCustomFps(uuid: string, fps: number): number {
  try {
    return native.setCustomFps(uuid, fps);
  } catch (e) {
    logFail('setCustomFps', e as Object);
    return -1;
  }
}

export function ffiToggleOption(uuid: string, name: string): number {
  try {
    return native.toggleOption(uuid, name);
  } catch (e) {
    logFail('toggleOption', e as Object);
    return -1;
  }
}

export function ffiGetToggleOption(uuid: string, name: string): number {
  try {
    return native.getToggleOption(uuid, name);
  } catch (e) {
    logFail('getToggleOption', e as Object);
    return -1;
  }
}

export function ffiUpdateSupportedDecodings(uuid: string): number {
  try {
    return native.updateSupportedDecodings(uuid);
  } catch (e) {
    logFail('updateSupportedDecodings', e as Object);
    return -1;
  }
}

export function ffiAlternativeCodecs(uuid: string): string {
  try {
    return native.alternativeCodecs(uuid);
  } catch (e) {
    logFail('alternativeCodecs', e as Object);
    return '';
  }
}

// ---------------------------------------------------------------------------
// Controlled side (被控端)
// ---------------------------------------------------------------------------

export function ffiServerStart(): number {
  try {
    return native.serverStart();
  } catch (e) {
    logFail('serverStart', e as Object);
    return -1;
  }
}

export function ffiServerStartService(): number {
  try {
    return native.serverStartService();
  } catch (e) {
    logFail('serverStartService', e as Object);
    return -1;
  }
}

export function ffiServerStopService(): number {
  try {
    return native.serverStopService();
  } catch (e) {
    logFail('serverStopService', e as Object);
    return -1;
  }
}

export function ffiGetServerId(): string {
  try {
    return native.getServerId();
  } catch (e) {
    logFail('getServerId', e as Object);
    return '';
  }
}

export function ffiGetServerPassword(): string {
  try {
    return native.getServerPassword();
  } catch (e) {
    logFail('getServerPassword', e as Object);
    return '';
  }
}

export function ffiUpdateServerPassword(): number {
  try {
    return native.updateServerPassword();
  } catch (e) {
    logFail('updateServerPassword', e as Object);
    return -1;
  }
}

export function ffiHasPermanentPassword(): number {
  try {
    return native.hasPermanentPassword();
  } catch (e) {
    logFail('hasPermanentPassword', e as Object);
    return -1;
  }
}

export function ffiServerAuthorize(id: number, accept: boolean): number {
  try {
    return native.serverAuthorize(id, accept);
  } catch (e) {
    logFail('serverAuthorize', e as Object);
    return -1;
  }
}

export function ffiServerCloseConnection(id: number): number {
  try {
    return native.serverCloseConnection(id);
  } catch (e) {
    logFail('serverCloseConnection', e as Object);
    return -1;
  }
}

export function ffiCmHandleIncomingVoiceCall(id: number, accept: boolean): number {
  try {
    return native.cmHandleIncomingVoiceCall(id, accept);
  } catch (e) {
    logFail('cmHandleIncomingVoiceCall', e as Object);
    return -1;
  }
}

export function ffiCmCloseVoiceCall(id: number): number {
  try {
    return native.cmCloseVoiceCall(id);
  } catch (e) {
    logFail('cmCloseVoiceCall', e as Object);
    return -1;
  }
}

// Offer a voice call to the controlling peer (controlled side initiates).
export function ffiCmRequestVoiceCall(id: number): number {
  try {
    return native.cmRequestVoiceCall(id);
  } catch (e) {
    logFail('cmRequestVoiceCall', e as Object);
    return -1;
  }
}

export function ffiServerSwitchPermission(id: number, name: string, enabled: boolean): number {
  try {
    return native.serverSwitchPermission(id, name, enabled);
  } catch (e) {
    logFail('serverSwitchPermission', e as Object);
    return -1;
  }
}

export function ffiServerSendChat(id: number, text: string): number {
  try {
    return native.serverSendChat(id, text);
  } catch (e) {
    logFail('serverSendChat', e as Object);
    return -1;
  }
}

export function ffiServerPushClipboard(text: string, html: string): number {
  try {
    return native.serverPushClipboard(text, html);
  } catch (e) {
    logFail('serverPushClipboard', e as Object);
    return -1;
  }
}

export function ffiCaptureStart(width: number, height: number): number {
  try {
    return native.captureStart(width, height);
  } catch (e) {
    logFail('captureStart', e as Object);
    return -1;
  }
}

export function ffiCaptureStop(): number {
  try {
    return native.captureStop();
  } catch (e) {
    logFail('captureStop', e as Object);
    return -1;
  }
}

export function ffiCaptureMic(enable: boolean): number {
  try {
    return native.captureMic(enable);
  } catch (e) {
    logFail('captureMic', e as Object);
    return -1;
  }
}

export function ffiCaptureRunning(): number {
  try {
    return native.captureRunning();
  } catch (e) {
    logFail('captureRunning', e as Object);
    return -1;
  }
}
