// example: https://github.com/rgov/js-theora-decoder/blob/main/index.html
// dev: copy decoder files from node/ogv/dist/* to project dir
// dist: .... to dist
/*
  OGVDemuxerOggW: 'ogv-demuxer-ogg-wasm.js',
  OGVDemuxerWebMW: 'ogv-demuxer-webm-wasm.js',
  OGVDecoderAudioOpusW: 'ogv-decoder-audio-opus-wasm.js',
  OGVDecoderAudioVorbisW: 'ogv-decoder-audio-vorbis-wasm.js',
  OGVDecoderVideoTheoraW: 'ogv-decoder-video-theora-wasm.js',
  OGVDecoderVideoVP8W: 'ogv-decoder-video-vp8-wasm.js',
  OGVDecoderVideoVP8MTW: 'ogv-decoder-video-vp8-mt-wasm.js',
  OGVDecoderVideoVP9W: 'ogv-decoder-video-vp9-wasm.js',
  OGVDecoderVideoVP9SIMDW: 'ogv-decoder-video-vp9-simd-wasm.js',
  OGVDecoderVideoVP9MTW: 'ogv-decoder-video-vp9-mt-wasm.js',
  OGVDecoderVideoVP9SIMDMTW: 'ogv-decoder-video-vp9-simd-mt-wasm.js',
  OGVDecoderVideoAV1W: 'ogv-decoder-video-av1-wasm.js',
  OGVDecoderVideoAV1SIMDW: 'ogv-decoder-video-av1-simd-wasm.js',
  OGVDecoderVideoAV1MTW: 'ogv-decoder-video-av1-mt-wasm.js',
  OGVDecoderVideoAV1SIMDMTW: 'ogv-decoder-video-av1-simd-mt-wasm.js',
*/

export function loadVp9() {
  window.OGVLoader.loadClass(
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
  window.OGVLoader.loadClass(
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