// example: https://github.com/rgov/js-theora-decoder/blob/main/index.html
// https://github.com/brion/ogv.js/releases, yarn add has no simd
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
import { simd } from "wasm-feature-detect";

export async function loadVp9(callback) {
  // Multithreading is used only if `options.threading` is true. 
  // This requires browser support for the new `SharedArrayBuffer` and `Atomics` APIs, 
  // currently available in Firefox and Chrome with experimental flags enabled.
  // 所有主流浏览器均默认于2018年1月5日禁用SharedArrayBuffer
  const isSIMD = await simd();
  console.log('isSIMD: ' + isSIMD);
  window.OGVLoader.loadClass(
    isSIMD ? "OGVDecoderVideoVP9SIMDW" : "OGVDecoderVideoVP9W",
    (videoCodecClass) => {
      window.videoCodecClass = videoCodecClass;
      videoCodecClass({ videoFormat: {} }).then((decoder) => {
        decoder.init(() => {
          callback(decoder);
        })
      })
    },
    { worker: true, threading: true }
  );
}