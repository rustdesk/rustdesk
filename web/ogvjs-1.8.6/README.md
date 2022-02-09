ogv.js
======

Media decoder and player for Ogg Vorbis/Opus/Theora and WebM VP8/VP9/AV1 video.

Based around libogg, libvorbis, libtheora, libopus, libvpx, libnestegg and dav1d compiled to JavaScript and WebAssembly with Emscripten.

## Updates

1.8.6 - 2022-01-12
* Bump to yuv-canvas
* Fix demo for removal of video-canvas mode

1.8.5 - 2022-01-11
* Remove unnecessary user-agent checks
* Remove flaky, obsolete support for faking CSS `object-fit`
* Remove experimental support for streaming `<canvas>` into `<video>`

1.8.4 - 2021-07-02
* Fix for fix for OGVLoader.base fix

1.8.3 - 2021-07-02
* Fixes for build with emscripten 2.0.25
* Fix for nextTick/setImmediate-style polyfill in front-end
* Provisional fix for OGVLoader.base not working with CDNs
    * the fallback code for loading a non-local worker had been broken with WebAssembly for some time, sorry!

1.8.2 - errored out

1.8.1 - 2021-02-18
* Fixed OGVCompat APIs to correctly return false without WebAssembly and Web Audio

1.8.0 - 2021-02-09
* Dropping IE support and Flash audio backend
    * Updated to stream-file 0.3.0
    * Updated to audio-feeder 0.5.0
    * The old IE 10/11 support _no longer works_ due to the Flash plugin being disabled, and so is being removed
* Drop es6-promise shim
    * Now requires WebAssembly, which requires native Promise support
* Build & fixes
    * Demo fixed (removed test files that are now offline)
    * Builds with emscripten 2.0.13
    * Requires latest meson from git pending a fix hitting release

1.7.0 - 2020-09-28
* Builds with emscripten's LLVM upstream backend
    * Updated to build with emscripten 2.0.4
* Reduced amount of memory used between GC runs by reusing frame buffers
* Removed `memoryLimit` option
    * JS, Wasm, and threaded Wasm builds now all use dynamic memory growth
* Updated dav1d
* Updated libvpx to 1.8.1
* Experimental SIMD builds of AV1 decoder optional, with `make SIMD=1`
    * These work in Chrome with the "WebAssembly SIMD" flag enabled in chrome://flags/
    * Significant speed boost when available.
    * Available with and without multithreading.
    * Must enable explicitly with `simd: true` in `options`.
* Experimental SIMD work for VP9 as well, incomplete.

1.6.1 - 2019-06-18
* playbackSpeed attribute now supported
* updated audio-feeder to 0.4.21;
    * mono audio is now less loud, matching native playback better
    * audio resampling now uses linear interpolation for upscaling
    * fix for IE in bundling scenarios that use strict mode
    * tempo change support thanks to a great patch from velochy!
* updated yuv-canvas to 1.2.6;
    * fixes for capturing WebGL canvas as MediaStream
* fixes for seeks on low frame rate video
* updated emscripten toolchain to 1.38.36
    * drop OUTLINING_LIMIT from AV1 JS build; doesn't work in newer emscripten and not really needed

1.6.0 - 2019-02-26
* experimental support for AV1 video in WebM
* update buildchain to emscripten 1.38.28
* fix a stray global
* starting to move to ES6 classes and modules
* building with babel for ES5/IE11 compat
* updated eslint
* updated yuv-canvas to 1.2.4; fixes for software GL rendering
* updated audio-feeder to 0.4.15; fixes for resampling and Flash perf
* retooled buffer copies
* sync fix for audio packets with discard padding
* clients can pass a custom `StreamFile` instance as `{stream:foo}` in options. This can be useful for custom streaming until MSE interfaces are ready.
* refactored WebM keyframe detection
* prefill the frame pipeline as well as the audio pipeline before starting audio
* removed BINARYEN_IGNORE_IMPLICIT_TRAPS=1 option which can cause intermittent breakages
* changed download streaming method to avoid data corruption problem on certain files
* fix for seek on very short WebM files
* fix for replay-after-end-of-playback in WebM

See more details and history in [CHANGES.md](https://github.com/brion/ogv.js/blob/master/CHANGES.md)

## Current status

Note that as of 2021 ogv.js works pretty nicely but may still have some packagine oddities with tools like webpack. It should work via CDNs again as of 1.8.2 if you can't or don't want to package locally, but this is not documented well yet. Improved documentation will come with the next major update & code cleanup!

Since August 2015, ogv.js can be seen in action [on Wikipedia and Wikimedia Commons](https://commons.wikimedia.org/wiki/Commons:Video) in Safari and IE/Edge where native Ogg and WebM playback is not available. (See [technical details on MediaWiki integration](https://www.mediawiki.org/wiki/Extension:TimedMediaHandler/ogv.js).)

See also a standalone demo with performance metrics at https://brionv.com/misc/ogv.js/demo/

* streaming: yes (with Range header)
* seeking: yes for Ogg and WebM (with Range header)
* color: yes
* audio: yes, with a/v sync (requires Web Audio or Flash)
* background threading: yes (video, audio decoders in Workers)
* [GPU accelerated drawing: yes (WebGL)](https://github.com/brion/ogv.js/wiki/GPU-acceleration)
* GPU accelerated decoding: no
* SIMD acceleration: no
* Web Assembly: yes (with asm.js fallback)
* multithreaded VP8, VP9, AV1: in development (set `options.threading` to `true`; requires flags to be enabled in Firefox 65 and Chrome 72, no support yet in Safari)
* controls: no (currently provided by demo or other UI harness)

Ogg and WebM files are fairly well supported.


## Goals

Long-form goal is to create a drop-in replacement for the HTML5 video and audio tags which can be used for basic playback of Ogg Theora and Vorbis or WebM media on browsers that don't support Ogg or WebM natively.

The API isn't quite complete, but works pretty well.


## Compatibility

ogv.js requires a fast JS engine with typed arrays, and Web Audio for audio playback.

The primary target browsers are (testing 360p/30fps and up):
* Safari 6.1-12 on Mac OS X 10.7-10.14
* Safari on iOS 10-11 64-bit

Older versions of Safari have flaky JIT compilers. IE 9 and below lack typed arrays, and IE 10/11 no longer support an audio channel since the Flash plugin was sunset.

(Note that Windows and Mac OS X can support Ogg and WebM by installing codecs or alternate browsers with built-in support, but this is not possible on iOS where all browsers are really Safari.)

Testing browsers (these support .ogv and .webm natively):
* Firefox 65
* Chrome 73


## Package installation

Pre-built releases of ogv.js are available as [.zip downloads from the GitHub releases page](https://github.com/brion/ogv.js/releases) and through the npm package manager.

You can load the `ogv.js` main entry point directly in a script tag, or bundle it through whatever build process you like. The other .js files must be made available for runtime loading, together in the same directory.

ogv.js will try to auto-detect the path to its resources based on the script element that loads ogv.js or ogv-support.js. If you load ogv.js through another bundler (such as browserify or MediaWiki's ResourceLoader) you may need to override this manually before instantiating players:

```
  // Path to ogv-demuxer-ogg.js, ogv-worker-audio.js, etc
  OGVLoader.base = '/path/to/resources';
```

To fetch from npm:

```
npm install ogv
```

The distribution-ready files will appear in 'node_modules/ogv/dist'.

To load the player library into your browserify or webpack project:

```
var ogv = require('ogv');

// Access public classes either as ogv.OGVPlayer or just OGVPlayer.
// Your build/lint tools may be happier with ogv.OGVPlayer!
ogv.OGVLoader.base = '/path/to/resources';
var player = new ogv.OGVPlayer();
```

## Usage

The `OGVPlayer` class implements a player, and supports a subset of the events, properties and methods from [HTMLMediaElement](https://developer.mozilla.org/en-US/docs/Web/API/HTMLMediaElement) and [HTMLVideoElement](https://developer.mozilla.org/en-US/docs/Web/API/HTMLVideoElement).

```
  // Create a new player with the constructor
  var player = new OGVPlayer();

  // Or with options
  var player = new OGVPlayer({
	debug: true,
	debugFilter: /demuxer/
  });

  // Now treat it just like a video or audio element
  containerElement.appendChild(player);
  player.src = 'path/to/media.ogv';
  player.play();
  player.addEventListener('ended', function() {
    // ta-da!
  });
```

To check for compatibility before creating a player, include `ogv-support.js` and use the `OGVCompat` API:

```
  if (OGVCompat.supported('OGVPlayer')) {
    // go load the full player from ogv.js and instantiate stuff
  }
```

This will check for typed arrays, web audio, blacklisted iOS versions, and super-slow/broken JIT compilers.

If you need a URL versioning/cache-buster parameter for dynamic loading of `ogv.js`, you can use the `OGVVersion` symbol provided by `ogv-support.js` or the even tinier `ogv-version.js`:

```
  var script = document.createElement('script');
  script.src = 'ogv.js?version=' + encodeURIComponent(OGVVersion);
  document.querySelector('head').appendChild(script);
```

## Distribution notes

Entry points:
* `ogv.js` contains the main runtime classes, including OGVPlayer, OGVLoader, and OGVCompat.
* `ogv-support.js` contains the OGVCompat class and OGVVersion symbol, useful for checking for runtime support before loading the main `ogv.js`.
* `ogv-version.js` contains only the OGVVersion symbol.

These entry points may be loaded directly from a script element, or concatenated into a larger project, or otherwise loaded as you like.

Further code modules are loaded at runtime, which must be available with their defined names together in a directory. If the files are not hosted same-origin to the web page that includes them, you will need to set up appropriate CORS headers to allow loading of the worker JS modules.

Dynamically loaded assets:
* `ogv-worker-audio.js`, `ogv-worker-video.js`, and `*.worker.js` are Worker entry points, used to run video and audio decoders in the background.
* `ogv-demuxer-ogg-wasm.js/.wasm` are used in playing .ogg, .oga, and .ogv files.
* `ogv-demuxer-webm-wasm.js/.wasm` are used in playing .webm files.
* `ogv-decoder-audio-vorbis-wasm.js/.wasm` and `ogv-decoder-audio-opus-wasm.js/.wasm` are used in playing both Ogg and WebM files containing audio.
* `ogv-decoder-video-theora-wasm.js/.wasm` are used in playing .ogg and .ogv video files.
* `ogv-decoder-video-vp8-wasm.js/.wasm` and `ogv-decoder-video-vp9-wasm.js/.wasm` are used in playing .webm video files.
* `*-mt.js/.wasm` are the multithreaded versions of some of the above modules. They have additional support files.

If you know you will never use particular formats or codecs you can skip bundling them; for instance if you only need to play Ogg files you don't need `ogv-demuxer-webm-wasm.js` or `ogv-decoder-video-vp8-wasm.js` which are only used for WebM.


## Performance

(This section is somewhat out of date.)

As of 2015, for SD-or-less resolution basic Ogg Theora decoding speed is reliable on desktop and newer high-end mobile devices; current high-end desktops and laptops can even reach HD resolutions. Older and low-end mobile devices may have difficulty on any but audio and the lowest-resolution video files.

WebM VP8/VP9 is slower, but works pretty well at a resolution step below Theora.

AV1 is slower still, and tops out around 360p for single-threaded decoding on a fast desktop or iOS device.

*Low-res targets*

I've gotten acceptable performance for Vorbis audio and 160p/15fps Theora files on 32-bit iOS devices: iPhone 4s, iPod Touch 5th-gen and iPad 3. These have difficulty at 240p and above, and just won't keep up with higher resolutions.

Meanwhile, newer 64-bit iPhones and iPads are comparable to low-end laptops, and videos at 360p and often 480p play acceptably. Since 32-bit and 64-bit iOS devices have the same user-agent, a benchmark must be used to approximately test minimum CPU speed.

(On iOS, Safari performs significantly better than some alternative browsers that are unable to enable the JIT due to use of the old UIWebView API. Chrome 49 and Firefox for iOS are known to work using the newer WKWebView API internally. Again, a benchmark must be used to detect slow performance, as the browser remains otherwise compatible.)


Windows on 32-bit ARM platforms is similar... IE 11 on Windows RT 8.1 on a Surface tablet (NVidia Tegra 3) does not work (crashes IE), while Edge on Windows 10 Mobile works ok at low resolutions, having trouble starting around 240p.


In both cases, a native application looms as a possibly better alternative. See [OGVKit](https://github.com/brion/OGVKit) and [OgvRt](https://github.com/brion/OgvRT) projects for experiments in those directions.


Note that at these lower resolutions, Vorbis audio and Theora video decoding are about equally expensive operations -- dual-core phones and tablets should be able to eke out a little parallelism here thanks to audio and video being in separate Worker threads.


*WebGL drawing acceleration*

Accelerated YCbCr->RGB conversion and drawing is done using WebGL on supporting browsers, or through software CPU conversion if not. This is abstracted in the [yuv-canvas](https://github.com/brion/yuv-canvas) package, now separately installable.

It may be possible to do further acceleration of actual decoding operations using WebGL shaders, but this could be ... tricky. WebGL is also only available on the main thread, and there are no compute shaders yet so would have to use fragment shaders.


## Difficulties

*Threading*

Currently the video and audio codecs run in worker threads by default, while the demuxer and player logic run on the UI thread. This seems to work pretty well.

There is some overhead in extracting data out of each emscripten module's heap and in the thread-to-thread communications, but the parallelism and smoother main thread makes up for it.

*Streaming download*

Streaming buffering is done by chunking the requests at up to a megabyte each, using the HTTP Range header. For cross-site playback, this requires CORS setup to whitelist the Range header! Chunks are downloaded as ArrayBuffers, so a chunk must be loaded in full before demuxing or playback can start.

Old versions of [Safari have a bug with Range headers](https://bugs.webkit.org/show_bug.cgi?id=82672) which is worked around as necessary with a 'cache-busting' URL string parameter.


*Seeking*

Seeking is implemented via the HTTP Range: header.

For Ogg files with keyframe indices in a skeleton index, seeking is very fast. Otherwise,  a bisection search is used to locate the target frame or audio position, which is very slow over the internet as it creates a lot of short-lived HTTP requests.

For WebM files with cues, efficient seeking is supported as well as of 1.1.2. WebM files without cues can be seeked in 1.5.5, but inefficiently via linear seek from the beginning. This is fine for small audio-only files, but might be improved for large files with a bisection in future.

As with chunked streaming, cross-site playback requires CORS support for the Range header.


*Audio output*

Audio output is handled through the [AudioFeeder](https://github.com/brion/audio-feeder) library, which encapsulates use of Web Audio API:

Firefox, Safari, Chrome, and Edge support the W3C Web Audio API.

IE is no longer supported; the workaround using Flash no longer works due to sunsetting of the Flash plugin.

A/V synchronization is performed on files with both audio and video, and seems to actually work. Yay!

Note that autoplay with audio doesn't work on iOS Safari due to limitations with starting audio playback from event handlers; if playback is started outside an event handler, the player will hang due to broken audio.

As of 1.1.1, muting before script-triggered playback allows things to work:

```
  player = new OGVPlayer();
  player.muted = true;
  player.src = 'path/to/file-with-audio.ogv';
  player.play();
```

You can then unmute the video in response to a touch or click handler. Alternately if audio is not required, do not include an audio track in the file.


*WebM*

WebM support was added in June 2015, with some major issues finally worked out in May 2016. Initial VP9 support was added in February 2017. It's pretty stable in production use at Wikipedia and is enabled by default as of October 2015.

Beware that performance of WebM VP8 is much slower than Ogg Theora, and VP9 is slightly slower still.

For best WebM decode speed, consider encoding VP8 with "profile 1" (simple deblocking filter) which will sacrifice quality modestly, mainly in high-motion scenes. When encoding with ffmpeg, this is the `-profile:v 1` option to the `libvpx` codec.

It is also recommended to use the `-slices` option for VP8, or `-tile-columns` for VP9, to maximize ability to use multithreaded decoding when available in the future.

*AV1*

WebM files containing the AV1 codec are supported as of 1.6.0 (February 2019) using the [dav1d](https://code.videolan.org/videolan/dav1d) decoder.

Currently this is experimental, and does not advertise support via `canPlayType`.

Performance is about 2-3x slower than VP8 or VP9, and may require bumping down a resolution step or two to maintain frame rate. There may be further optimizations that can be done to improve this a bit, but the best improvements will come from future improvements to WebAssembly multithreading and SIMD.

Currently AV1 in MP4 container is not supported.

## Upstream library notes

We've experimented with tremor (libivorbis), an integer-only variant of libvorbis. This actually does *not* decode faster, but does save about 200kb off our generated JavaScript, presumably thanks to not including an encoder in the library. However on slow devices like iPod Touch 5th-generation, it makes a significant negative impact on the decode time so we've gone back to libvorbis.

The Ogg Skeleton library (libskeleton) is a bit ... unfinished and is slightly modified here.

libvpx is slightly modified to work around emscripten threading limitations in the VP8 decoder.


## WebAssembly

WebAssembly (Wasm) builds are used exclusively as of 1.8.0, as Safari's Wasm support is pretty well established now and IE no longer works due to the Flash plugin deprecation.


## Multithreading

Experimental multithreaded VP8, VP9, and AV1 decoding up to 4 cores is in development, requiring emscripten 1.38.27 to build.

Multithreading is used only if `options.threading` is true. This requires browser support for the new `SharedArrayBuffer` and `Atomics` APIs, currently available in Firefox and Chrome with experimental flags enabled.

Threading currently requires WebAssembly; JavaScript builds are possible but perform poorly.

Speedups will only be noticeable when using the "slices" or "token partitions" option for VP8 encoding, or the "tile columns" option for VP9 encoding.

If you are making a slim build and will not use the `threading` option, you can leave out the `*-mt.*` files.


## Building JS components

Building ogv.js is known to work on Mac OS X and Linux (tested Fedora 29 and Ubuntu 18.10 with Meson manually updated).

1. You will need autoconf, automake, libtool, pkg-config, meson, ninja, and node (nodejs). These can be installed through Homebrew on Mac OS X, or through distribution-specific methods on Linux. For meson, you may need a newer version than your distro packages -- install it manually with `pip3` or from source.
2. Install [Emscripten](http://kripken.github.io/emscripten-site/docs/getting_started/Tutorial.html); currently building with 2.0.13.
3. `git submodule update --init`
4. Run `npm install` to install build utilities
5. Run `make js` to configure and build the libraries and the C wrapper


## Building the demo

If you did all the setup above, just run `make demo` or `make`. Look in build/demo/ and enjoy!


## License

libogg, libvorbis, libtheora, libopus, nestegg, libvpx, and dav1d are available under their respective licenses, and the JavaScript and C wrapper code in this repo is licensed under MIT.

Based on build scripts from https://github.com/devongovett/ogg.js

See [AUTHORS.md](https://github.com/brion/ogv.js/blob/master/AUTHORS.md) and/or the git history for a list of contributors.
