import * as proto from '../message.js';

export default class Websock {
    constructor() {
        this._websocket = null;  // WebSocket object
        this._eventHandlers = {
            message: (msg) => { },
            open: () => { },
            close: () => { },
            error: () => { }
        };
        this._next_yuv = null;
        this._next_rgb = null;
    }

    send(msg) {
        if (msg instanceof Object) msg = proto.encodeMessage(msg);
        this._websocket.send(msg);
    }

    // Event Handlers
    off(evt) {
        this._eventHandlers[evt] = () => { };
    }

    on(evt, handler) {
        this._eventHandlers[evt] = handler;
    }

    init() {
        this._websocket = null;
    }

    open(uri, protocols) {
        this.init();

        this._websocket = new WebSocket(uri, protocols);

        this._websocket.onmessage = this._recv_message.bind(this);
        this._websocket.binaryType = 'arraybuffer';
        this._websocket.onopen = () => {
            console.debug('>> WebSock.onopen');
            if (this._websocket.protocol) {
                console.info("Server choose sub-protocol: " + this._websocket.protocol);
            }

            this._eventHandlers.open();
            console.debug("<< WebSock.onopen");
        };
        this._websocket.onclose = (e) => {
            console.debug(">> WebSock.onclose");
            this._eventHandlers.close(e);
            console.debug("<< WebSock.onclose");
        };
        this._websocket.onerror = (e) => {
            console.debug(">> WebSock.onerror: " + e);
            this._eventHandlers.error(e);
            console.debug("<< WebSock.onerror: " + e);
        };
    }

    close() {
        if (this._websocket) {
            if ((this._websocket.readyState === WebSocket.OPEN) ||
                (this._websocket.readyState === WebSocket.CONNECTING)) {
                console.info("Closing WebSocket connection");
                this._websocket.close();
            }

            this._websocket.onmessage = () => { };
        }
    }

    _recv_message(e) {
        if (e.data instanceof window.ArrayBuffer) {
            let bytes = new Uint8Array(e.data);
            if (this._next_yuv) {
                const yuv = this._next_yuv;
                const { compress, stride } = yuv.format;
                if (compress) {
                    bytes = window.simple_zstd.decompress(bytes);
                }
                if (!yuv.y) {
                    yuv.y = { bytes, stride: stride };
                } else if (!yuv.u) {
                    yuv.u = { bytes, stride: stride >> 1 };
                } else {
                    yuv.v = { bytes, stride: stride >> 1 };
                    delete yuv.format.stride;
                    this._eventHandlers.message({ video_frame: { yuv } });
                    this._next_yuv = null;
                }
            } else if (this._next_rgb) {
                if (this._next_rgb.format.compress) {
                    bytes = window.simple_zstd.decompress(bytes);
                }
                this._eventHandlers.message({ video_frame: { rgb: bytes } });
                this._next_rgb = null;
            } else {
                const msg = proto.decodeMessage(bytes);
                let vf = msg.video_frame;
                if (vf) {
                    const { yuv, rgb } = vf;
                    if (yuv) {
                        this._next_yuv = { format: yuv };
                    } else if (rgb) {
                        this._next_rgb = { format: rgb };
                    }
                    return;
                } else {
                    this._eventHandlers.message(msg);
                }
            }
        }
    }
}