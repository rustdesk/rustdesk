[![cargo version](https://img.shields.io/crates/v/nokhwa.svg)](https://crates.io/crates/nokhwa) [![docs.rs version](https://img.shields.io/docsrs/nokhwa)](https://docs.rs/nokhwa/latest/nokhwa/)
# nokhwa
Nokhwa(녹화): Korean word meaning "to record".

A Simple-to-use, cross-platform Rust Webcam Capture Library

## Using nokhwa
Nokhwa can be added to your crate by adding it to your `Cargo.toml`:
```toml
[dependencies.nokhwa]
version = "0.10.0"
# Use the native input backends, enable WGPU integration
features = ["input-native", "output-wgpu"]
```

Most likely, you will only use functionality provided by the `Camera` struct. If you need lower-level access, you may instead opt to use the raw capture backends found at `nokhwa::backends::capture::*`.

## Example
```rust
// first camera in system
let index = CameraIndex::index(0); 
// request the absolute highest resolution CameraFormat that can be decoded to RGB.
let requested = RequestedFormat::<RgbFormat>::new(RequestedFormatType::AbsoluteHighestFrameRate);
// make the camera
let mut camera = Camera::new(index, requested).unwrap();

// get a frame
let frame = camera.frame().unwrap();
println!("Captured Single Frame of {}", frame.buffer().len());
// decode into an ImageBuffer
let decoded = frame.decode_image::<RgbFormat>().unwrap();
println!("Decoded Frame of {}", decoded.len());
```

A command line app made with `nokhwa` can be found in the `examples` folder.

## API Support
The table below lists current Nokhwa API support.
- The `Backend` column signifies the backend.
- The `Input` column signifies reading frames from the camera
- The `Query` column signifies system device list support
- The `Query-Device` column signifies reading device capabilities
- The `Platform` column signifies what Platform this is availible on.

 | Backend                              | Input              | Query             | Query-Device       | Platform            |
 |-----------------------------------------|-------------------|--------------------|-------------------|--------------------|
 | Video4Linux(`input-native`)          | ✅                 | ✅                 | ✅                | Linux               |
 | MSMF(`input-native`)                 | ✅                 | ✅                 | ✅                | Windows             |
 | AVFoundation(`input-native`)   | ✅                 | ✅                 | ✅                | Mac                 |
 | OpenCV(`input-opencv`)^              | ✅                 | ❌                 | ❌                | Linux, Windows, Mac |
 | WASM(`input-wasm`)                | ✅                 | ✅                 | ✅                | Browser(Web)        |

 ✅: Working, 🔮 : Experimental, ❌ : Not Supported, 🚧: Planned/WIP

  ^ = May be bugged. Also supports IP Cameras. 

## Feature
The default feature includes nothing. Anything starting with `input-*` is a feature that enables the specific backend. 

`input-*` features:
 - `input-native`: Uses either V4L2(Linux), MSMF(Windows), or AVFoundation(Mac OS)
 - `input-opencv`: Enables the `opencv` backend. (cross-platform) 
 - `input-jscam`: Enables the use of the `JSCamera` struct, which uses browser APIs. (Web)

Conversely, anything that starts with `output-*` controls a feature that controls the output of something (usually a frame from the camera)

`output-*` features:
 - `output-wgpu`: Enables the API to copy a frame directly into a `wgpu` texture.
 - `output-threaded`: Enable the threaded/callback based camera. 

Other features:
 - `decoding`: Enables `mozjpeg` decoding. Enabled by default.
 - `docs-only`: Documentation feature. Enabled for docs.rs builds.
 - `docs-nolink`: Build documentation **without** linking to any libraries. Enabled for docs.rs builds.
 - `test-fail-warning`: Fails on warning. Enabled in CI.

You many want to pick and choose to reduce bloat.

## Issues
If you are making an issue, please make sure that
 - It has not been made yet
 - Attach what you were doing, your environment, steps to reproduce, and backtrace.
Thank you!

## Contributing
Contributions are welcome!
 - Please `rustfmt` all your code and adhere to the clippy lints (unless necessary not to do so)
 - Please limit use of `unsafe`
 - All contributions are under the Apache 2.0 license unless otherwise specified

## Minimum Service Rust Version
`nokhwa` may build on older versions of `rustc`, but there is no guarantee except for the latest stable rust. 

## Sponsors
- $40/mo sponsors:
  - [erlend-sh](https://github.com/erlend-sh)
  - [DanielMSchmidt](https://github.com/DanielMSchmidt)
- $5/mo sponsors:
  - [remifluff](https://github.com/remifluff)
  - [gennyble](https://github.com/gennyble)
  
Please consider [donating](https://github.com/sponsors/l1npengtul)! It helps me not look like a failure to my parents!
