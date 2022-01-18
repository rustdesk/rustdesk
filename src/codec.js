import { OGVLoader } from "ogv";

// example: https://github.com/rgov/js-theora-decoder/blob/main/index.html
// dev: copy decoder files from node/ogv/dist/* to project dir
// dist: .... to dist

export function loadVp9() {
    OGVLoader.loadClass(
        "OGVDecoderVideoVP9W",
        (videoCodecClass) => {
            videoCodecClass().then((decoder) => {
                decoder.init(() => {
                    onVp9Ready(decoder)
                })
            })
        },
        { worker: true }
    );
}

export function loadOpus() {
    OGVLoader.loadClass(
        "OGVDecoderAudioOpusW",
        (audioCodecClass) => {
            audioCodecClass().then((decoder) => {
                decoder.init(() => {
                    onOpusReady(decoder)
                })
            })
        },
        { worker: true }
    );
}

async function onVp9Ready(decoder) {
    console.log("Vp9 decoder ready");

    /*
    decoder.processFrame(buffer, () => {
        player.drawFrame(decoder.frameBuffer)
    })
    */
}

async function onOpusReady(decoder) {
    console.log("Opus decoder ready");
}