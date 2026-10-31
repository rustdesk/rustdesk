// RustDesk OHOS FFI (libentry.so -> librustdesk.so via dlopen).
// Signature contract: docs/harmonyos-migration/ohos-controller-ffi.md
//
// All session-scoped functions take the session uuid returned by sessionNew
// as the first argument, mirroring the C ABI one to one.

export const init: (home: string, appDir: string, customConfig: string) => number;
export const setCallbacks: (onEvent: (json: string) => void,
                            onClipboard: (text: string) => void) => number;
export const initVideo: (surfaceId: string) => number;
export const sessionNew: (peerId: string, sessionKind: number, password: string,
                          forceRelay: boolean) => string;
export const sessionStart: (uuid: string) => number;
export const sessionLogin: (uuid: string, password: string, remember: boolean) => number;
export const sessionSend2fa: (uuid: string, code: string, trustThisDevice: boolean) => number;
export const sessionReconnect: (uuid: string, forceRelay: boolean) => number;
export const sessionClose: (uuid: string) => number;
export const sendMouse: (uuid: string, mask: number, x: number, y: number, alt: boolean,
                         ctrl: boolean, shift: boolean, command: boolean) => number;
export const inputKey: (uuid: string, name: string, down: boolean, press: boolean,
                        alt: boolean, ctrl: boolean, shift: boolean, command: boolean) => number;
export const inputString: (uuid: string, value: string) => number;
export const refreshVideo: (uuid: string) => number;
export const switchDisplay: (uuid: string, display: number) => number;
export const sendClipboard: (uuid: string, text: string) => number;
export const sendFiles: (uuid: string, actId: number, path: string, to: string,
                         fileNum: number, includeHidden: boolean, isRemote: boolean) => number;
export const setConfirmOverrideFile: (uuid: string, actId: number, fileNum: number,
                                      needOverride: boolean, remember: boolean,
                                      isUpload: boolean) => number;
export const cancelJob: (uuid: string, actId: number) => number;
export const removeFile: (uuid: string, actId: number, path: string, fileNum: number,
                          isRemote: boolean) => number;
export const readDir: (uuid: string, path: string, includeHidden: boolean,
                       isRemote: boolean) => number;
export const getOption: (key: string) => string;
export const setOption: (key: string, value: string) => number;
export const recordScreen: (uuid: string, start: boolean) => number;
export const requestVoiceCall: (uuid: string) => number;
export const closeVoiceCall: (uuid: string) => number;
export const answerVoiceCall: (uuid: string, accept: boolean) => number;
export const sendClipboardImagePng: (uuid: string, pngBase64: string) => number;
export const sessionGetOption: (uuid: string, key: string) => string;
export const sessionSetOption: (uuid: string, key: string, value: string) => number;
export const saveImageQuality: (uuid: string, value: string) => number;
export const saveCustomImageQuality: (uuid: string, quality: number) => number;
export const setCustomFps: (uuid: string, fps: number) => number;
export const toggleOption: (uuid: string, name: string) => number;
export const getToggleOption: (uuid: string, name: string) => number;
export const updateSupportedDecodings: (uuid: string) => number;
export const alternativeCodecs: (uuid: string) => string;
export const version: () => string;

// Controlled side (被控端)
export const serverStart: () => number;
export const serverStartService: () => number;
export const serverStopService: () => number;
export const getServerId: () => string;

export const getClients: () => string;
export const getServerPassword: () => string;
export const updateServerPassword: () => number;
export const hasPermanentPassword: () => number;
export const serverAuthorize: (id: number, accept: boolean) => number;
export const serverCloseConnection: (id: number) => number;
export const cmHandleIncomingVoiceCall: (id: number, accept: boolean) => number;
export const cmCloseVoiceCall: (id: number) => number;
export const cmRequestVoiceCall: (id: number) => number;
export const serverSwitchPermission: (id: number, name: string, enabled: boolean) => number;
export const serverSendChat: (id: number, text: string) => number;
export const serverPushClipboard: (text: string, html: string) => number;
export const captureStart: (width: number, height: number) => number;
export const captureStop: () => number;
export const captureMic: (enable: boolean) => number;
export const captureRunning: () => number;
