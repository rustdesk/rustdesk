// RustDesk OHOS FFI (libentry.so -> librustdesk.so via dlopen).
// Signature contract: docs/harmonyos-migration/ohos-controller-ffi.md
//
// All session-scoped functions take the session uuid returned by sessionNew
// as the first argument, mirroring the C ABI one to one.

export const init: (home: string, appDir: string, customConfig: string) => number;
export const setCallbacks: (onEvent: (json: string) => void,
                            onClipboard: (text: string) => void) => number;
export const initVideo: (surfaceId: string) => number;
export const sessionNew: (peerId: string, isFileTransfer: boolean, password: string,
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
export const version: () => string;
