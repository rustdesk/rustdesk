# [flutter_rust_bridge](https://github.com/fzyzcjy/flutter_rust_bridge): High-level memory-safe binding generator for Flutter/Dart <-> Rust

[![Rust Package](https://img.shields.io/crates/v/flutter_rust_bridge.svg)](https://crates.io/crates/flutter_rust_bridge)
[![Flutter Package](https://img.shields.io/pub/v/flutter_rust_bridge.svg)](https://pub.dev/packages/flutter_rust_bridge)
[![Stars](https://img.shields.io/github/stars/fzyzcjy/flutter_rust_bridge)](https://github.com/fzyzcjy/flutter_rust_bridge)
[![CI](https://github.com/fzyzcjy/flutter_rust_bridge/actions/workflows/ci.yaml/badge.svg)](https://github.com/fzyzcjy/flutter_rust_bridge/actions/workflows/ci.yaml)
[![Example](https://github.com/fzyzcjy/flutter_rust_bridge/actions/workflows/post_release.yaml/badge.svg)](https://github.com/fzyzcjy/flutter_rust_bridge/actions/workflows/post_release.yaml)
[![Codacy Badge](https://api.codacy.com/project/badge/Grade/6afbdad19e7245adbf9e9771777be3d7)](https://app.codacy.com/gh/fzyzcjy/flutter_rust_bridge?utm_source=github.com&utm_medium=referral&utm_content=fzyzcjy/flutter_rust_bridge&utm_campaign=Badge_Grade_Settings)

![Logo](https://github.com/fzyzcjy/flutter_rust_bridge/raw/master/book/logo.png)

Want to combine the best between [Flutter](https://flutter.dev/), a cross-platform hot-reload rapid-development UI toolkit, and [Rust](https://www.rust-lang.org/), a language empowering everyone to build reliable and efficient software? Here it comes!

## 🚀 Advantages

* **Memory-safe**: Never need to think about malloc/free.
* **Feature-rich**: `enum`s with values, platform-optimized `Vec`, possibly recursive `struct`, zero-copy big arrays, `Stream` (iterator) abstraction, error (`Result`) handling, cancellable tasks, concurrency control, and more. See full features [here](https://fzyzcjy.github.io/flutter_rust_bridge/feature.html). 
* **Async programming**: Rust code will never block the Flutter. Call Rust naturally from Flutter's main isolate (thread).
* **Lightweight**: This is not a huge framework that includes everything, so you are free to use your favorite Flutter and Rust libraries. <sub>For example, state-management with Flutter library (e.g. MobX) can be elegant and simple (contrary to implementing in Rust); implementing a photo manipulation algorithm in Rust will be fast and safe (countrary to implementing in Flutter).</sub>
* **Cross-platform**: Android, iOS, Windows, Linux, MacOS ([Web](https://github.com/fzyzcjy/flutter_rust_bridge/issues/315) coming soon)
* **Easy to code-review & convince yourself**: This package simply simulates how humans  write boilerplate code. If you want to convince yourself (or your team) that it is safe, there is not much code to look at. No magic at all! ([More about](https://fzyzcjy.github.io/flutter_rust_bridge/safety.html) safety concerns.)
* **Fast**: It is only a thin (though feature-rich) wrapper, without overhead such as protobuf serialization, thus performant. (More [benchmarks](https://github.com/fzyzcjy/flutter_rust_bridge/issues/318#issuecomment-1034536815) later) <small>(Throw away components like thread-pool to make it even faster)</small>
* **Pure-Dart compatible:** Despite the name, this package is 100% compatible with [pure](https://github.com/fzyzcjy/flutter_rust_bridge/blob/master/frb_example/pure_dart/README.md) Dart.

## 💡 User Guide

Check out [the user guide](https://fzyzcjy.github.io/flutter_rust_bridge/) for [show-me-the-code](https://fzyzcjy.github.io/flutter_rust_bridge/quickstart.html), [tutorials](https://fzyzcjy.github.io/flutter_rust_bridge/tutorial_with_flutter.html), [features](https://fzyzcjy.github.io/flutter_rust_bridge/feature.html) and much more.

## 📎 P.S. Convenient Flutter tests

If you want to write and debug tests in Flutter conveniently, with action history, time travelling, screenshots, rapid re-execution, video recordings, interactive mode and more, here is my another open-source library: https://github.com/fzyzcjy/flutter_convenient_test.

## ✨ Contributors

<!-- ALL-CONTRIBUTORS-BADGE:START - Do not remove or modify this section -->
[![All Contributors](https://img.shields.io/badge/all_contributors-18-orange.svg?style=flat-square)](#contributors-)
<!-- ALL-CONTRIBUTORS-BADGE:END -->

Thanks goes to these wonderful people ([emoji key](https://allcontributors.org/docs/en/emoji-key) following [all-contributors](https://github.com/all-contributors/all-contributors) specification):

<!-- ALL-CONTRIBUTORS-LIST:START - Do not remove or modify this section -->
<!-- prettier-ignore-start -->
<!-- markdownlint-disable -->
<table>
  <tr>
    <td align="center"><a href="https://github.com/fzyzcjy"><img src="https://avatars.githubusercontent.com/u/5236035?v=4?s=100" width="100px;" alt=""/><br /><sub><b>fzyzcjy</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=fzyzcjy" title="Code">💻</a> <a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=fzyzcjy" title="Documentation">📖</a> <a href="#example-fzyzcjy" title="Examples">💡</a> <a href="#ideas-fzyzcjy" title="Ideas, Planning, & Feedback">🤔</a> <a href="#maintenance-fzyzcjy" title="Maintenance">🚧</a></td>
    <td align="center"><a href="https://github.com/Desdaemon"><img src="https://avatars.githubusercontent.com/u/36768030?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Viet Dinh</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=Desdaemon" title="Code">💻</a> <a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=Desdaemon" title="Tests">⚠️</a> <a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=Desdaemon" title="Documentation">📖</a></td>
    <td align="center"><a href="https://github.com/SecondFlight"><img src="https://avatars.githubusercontent.com/u/6700184?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Joshua Wade</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=SecondFlight" title="Code">💻</a></td>
    <td align="center"><a href="https://github.com/smw-wagnerma"><img src="https://avatars.githubusercontent.com/u/66412697?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Marcel</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=smw-wagnerma" title="Code">💻</a></td>
    <td align="center"><a href="https://github.com/rustui"><img src="https://avatars.githubusercontent.com/u/90625190?v=4?s=100" width="100px;" alt=""/><br /><sub><b>rustui</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=rustui" title="Documentation">📖</a></td>
    <td align="center"><a href="https://adventures.michaelfbryan.com/"><img src="https://avatars.githubusercontent.com/u/17380079?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Michael Bryan</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=Michael-F-Bryan" title="Code">💻</a></td>
    <td align="center"><a href="https://bus710.net"><img src="https://avatars.githubusercontent.com/u/8920680?v=4?s=100" width="100px;" alt=""/><br /><sub><b>bus710</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=bus710" title="Documentation">📖</a></td>
  </tr>
  <tr>
    <td align="center"><a href="https://scholar.google.com/citations?user=RbAto7EAAAAJ"><img src="https://avatars.githubusercontent.com/u/1213857?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Sebastian Urban</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=surban" title="Code">💻</a></td>
    <td align="center"><a href="https://github.com/trobanga"><img src="https://avatars.githubusercontent.com/u/8888869?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Daniel</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=trobanga" title="Code">💻</a></td>
    <td align="center"><a href="https://github.com/AlienKevin"><img src="https://avatars.githubusercontent.com/u/22850071?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Kevin Li</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=AlienKevin" title="Code">💻</a> <a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=AlienKevin" title="Documentation">📖</a></td>
    <td align="center"><a href="https://valeth.me"><img src="https://avatars.githubusercontent.com/u/3198362?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Patrick Auernig</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=valeth" title="Code">💻</a></td>
    <td align="center"><a href="https://antonok.com"><img src="https://avatars.githubusercontent.com/u/22821309?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Anton Lazarev</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=antonok-edm" title="Code">💻</a></td>
    <td align="center"><a href="https://github.com/Unoqwy"><img src="https://avatars.githubusercontent.com/u/65187632?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Unoqwy</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=Unoqwy" title="Code">💻</a></td>
    <td align="center"><a href="https://feber.dev"><img src="https://avatars.githubusercontent.com/u/1727318?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Febrian Setianto</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=feber" title="Documentation">📖</a></td>
  </tr>
  <tr>
    <td align="center"><a href="https://github.com/Syndim"><img src="https://avatars.githubusercontent.com/u/835035?v=4?s=100" width="100px;" alt=""/><br /><sub><b>syndim</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=syndim" title="Code">💻</a></td>
    <td align="center"><a href="https://github.com/sagudev"><img src="https://avatars.githubusercontent.com/u/16504129?v=4?s=100" width="100px;" alt=""/><br /><sub><b>sagu</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=sagudev" title="Code">💻</a> <a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=sagudev" title="Documentation">📖</a></td>
    <td align="center"><a href="https://bandism.net/"><img src="https://avatars.githubusercontent.com/u/22633385?v=4?s=100" width="100px;" alt=""/><br /><sub><b>Ikko Ashimine</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=eltociear" title="Documentation">📖</a></td>
    <td align="center"><a href="https://github.com/alanlzhang"><img src="https://avatars.githubusercontent.com/u/59032810?v=4?s=100" width="100px;" alt=""/><br /><sub><b>alanlzhang</b></sub></a><br /><a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=alanlzhang" title="Code">💻</a> <a href="https://github.com/fzyzcjy/flutter_rust_bridge/commits?author=alanlzhang" title="Documentation">📖</a></td>
  </tr>
</table>

<!-- markdownlint-restore -->
<!-- prettier-ignore-end -->

<!-- ALL-CONTRIBUTORS-LIST:END -->

More specifically, thanks for all these contributions:

* [Desdaemon](https://github.com/Desdaemon): Support not only simple enums but also enums with fields which gets translated to native enum or freezed class in Dart. Support the Option type as nullable types in Dart. Support Vec of Strings type. Support comments in code. Add marker attributes for future usage. Add Linux and Windows support for with-flutter example, and make CI works for that. Avoid parameter collision. Overhaul the documentation and add several chapters to demonstrate configuring a Flutter+Rust project in all five platforms. Refactor command module.
* [SecondFlight](https://github.com/SecondFlight): Allow structs and enums to be imported from other files within the crate by creating source graph. Auto-create relavent dir.
* [Unoqwy](https://github.com/Unoqwy): Add struct mirrors, such that types in the external crates can be imported and used without redefining and copying.
* [antonok-edm](https://github.com/antonok-edm): Avoid converting syn types to strings before parsing to improve code and be more robust.
* [sagudev](https://github.com/sagudev): Make code generator a `lib`. Add error types. Depend on `cbindgen`. Fix LLVM paths. Update deps. Fix CI errors.
* [surban](https://github.com/surban): Support unit return type. Skip unresolvable modules. Ignore prefer_const_constructors. Non-final Dart fields.
* [trobanga](https://github.com/trobanga): Add support for `[T;N]` structs. Add `usize` support. Add a cmd argument. Separate dart tests.
* [AlienKevin](https://github.com/AlienKevin): Add flutter example for macOS. Add doc for Android NDK bug.
* [alanlzhang](https://github.com/alanlzhang): Add generation for Dart metadata.
* [efc-mw](https://github.com/efc-mw): Improve Windows encoding handling.
* [valeth](https://github.com/valeth): Rename callFfi's port.
* [Michael-F-Bryan](https://github.com/Michael-F-Bryan): Detect broken bindings.
* [bus710](https://github.com/bus710): Add a case in troubleshooting.
* [Syndim](https://github.com/Syndim): Add a bracket to box.
* [feber](https://github.com/feber): Fix doc link.
* [rustui](https://github.com/rustui): Fix a typo.
* [eltociear](https://github.com/eltociear): Fix a typo.

