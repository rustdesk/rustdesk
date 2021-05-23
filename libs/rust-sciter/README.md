# Rust bindings for Sciter

[![Build status](https://ci.appveyor.com/api/projects/status/cbrisyh792mmmd08/branch/master?svg=true)](https://ci.appveyor.com/project/pravic/rust-sciter)
[![Build Status](https://img.shields.io/travis/sciter-sdk/rust-sciter/master.svg)](https://travis-ci.org/sciter-sdk/rust-sciter)
[![Minimum supported Rust version](https://img.shields.io/badge/rustc-1.38+-green.svg)](https://github.com/sciter-sdk/rust-sciter/commits/master/.travis.yml)
[![Current Version](https://img.shields.io/crates/v/sciter-rs.svg)](https://crates.io/crates/sciter-rs)
[![Documentation](https://docs.rs/sciter-rs/badge.svg)](https://docs.rs/sciter-rs)
[![License](https://img.shields.io/crates/l/sciter-rs.svg)](https://crates.io/crates/sciter-rs)
[![Join the forums at https://sciter.com/forums](https://img.shields.io/badge/forum-sciter.com-orange.svg)](https://sciter.com/forums)

Check [this page](https://sciter.com/developers/sciter-sdk-bindings/) for other language bindings (Delphi / D / Go / .NET / Python / Rust).

----


## Introduction

Sciter is an embeddable [multiplatform](https://sciter.com/sciter/crossplatform/) HTML/CSS/script engine with GPU accelerated rendering designed to render modern desktop application UI. It's a compact, single dll/dylib/so file (4-8 mb) engine without any additional dependencies.


## Screenshots

Check the [screenshot gallery](https://github.com/oskca/sciter#sciter-desktop-ui-examples) of desktop UI examples
and [DirectX UI integration](https://github.com/pravic/rust-gfx-sciter) via [Rust GFX](https://github.com/gfx-rs/gfx).


## Description

Physically Sciter is a mono library which contains:

* [HTML and CSS](https://sciter.com/developers/for-web-programmers/) rendering engine based on the H-SMILE core used in [HTMLayout](https://terrainformatica.com/a-homepage-section/htmlayout/),
* JavaScript alike [Scripting engine](https://sciter.com/developers/sciter-docs/) – core of [TIScript](https://sciter.com/developers/for-web-programmers/tiscript-vs-javascript/) which by itself is based on [c-smile](https://c-smile.sourceforge.net/) engine,
* Persistent [Database](https://sciter.com/docs/content/script/Storage.htm) (a.k.a. [JSON DB](https://terrainformatica.com/2006/10/what-the-hell-is-that-json-db/)) based on excellent DB products of [Konstantin Knizhnik](http://garret.ru/databases.html).
* [Graphics](https://sciter.com/docs/content/sciter/Graphics.htm) module that uses native graphics primitives provided by supported platforms: Direct2D on Windows 7 and above, GDI+ on Windows XP, CoreGraphics on MacOS, Cairo on Linux/GTK. Yet there is an option to use built-in [Skia/OpenGL](https://skia.org/) backend on each platform.
* Network communication module, it relies on platform HTTP client primitives and/or [Libcurl](https://curl.haxx.se/).


Internally it contains the following modules:

* **CSS** – CSS parser and the collection of parsed CSS rules, etc.
* **HTML DOM** – HTML parser and DOM tree implementation.
* **layout managers** – collection of various layout managers – text layout, default block layout, flex layouts. Support of positioned floating elements is also here. This module does the layout calculations heavy lifting. This module is also responsible for the rendering of layouts.
* **input behaviors** – a collection of built-in behaviors – code behind "active" DOM elements: `<input>`, `<select>`, `<textarea>`, etc.
* **script module** – source-to-bytecode compiler and virtual machine (VM) with compacting garbage collector (GC). This module also contains runtime implementation of standard classes and objects: Array, Object, Function and others.
* **script DOM** – runtime classes that expose DOM and DOM view (a.k.a. window) to the script.
* **graphics abstraction layer** – abstract graphics implementation that isolates the modules mentioned above from the particular platform details:
    * Direct2D/DirectWrite graphics backend (Windows);
    * GDI+ graphics backend (Windows);
    * CoreGraphics backend (Mac OS X);
    * Cairo backend (GTK on all Linux platforms);
    * Skia/OpenGL backend (all platforms)
* **core primitives** – set of common primitives: string, arrays, hash maps and so on.


Sciter supports all standard elements defined in HTML5 specification [with some additions](https://sciter.com/developers/for-web-programmers/). CSS is extended to better support the Desktop UI development, e.g. flow and flex units, vertical and horizontal alignment, OS theming.

[Sciter SDK](https://sciter.com/download/) comes with a demo "browser" with a builtin DOM inspector, script debugger and documentation viewer:

![Sciter tools](https://sciter.com/wp-content/uploads/2015/10/dom-tree-in-inspector-640x438.png)

Check <https://sciter.com> website and its [documentation resources](https://sciter.com/developers/) for engine principles, architecture and more.


## Getting started:

1. Download the [Sciter SDK](https://sciter.com/download/) and extract it somewhere.
2. Add the corresponding target platform binaries to PATH (`bin.win`, `bin.osx` or `bin.lnx`).
3. If you do not already have it installed, you need GTK 3 development tools installed to continue:
    sudo apt-get install libgtk-3-dev
4. Build the crate and run a minimal sciter sample: `cargo run --example minimal`.
5. For your apps add the following dependency to the Cargo.toml: `sciter-rs = "*"`.


## Brief look:

Here is a minimal sciter app:

```rust
extern crate sciter;

fn main() {
    let mut frame = sciter::Window::new();
    frame.load_file("minimal.htm");
    frame.run_app();
}
```

It looks similar to this:

![Minimal sciter sample](https://i.imgur.com/ojcM5JJ.png)

### Interoperability

In respect of [tiscript](https://www.codeproject.com/Articles/33662/TIScript-language-a-gentle-extension-of-JavaScript) functions calling:
```rust
use sciter::{Element, Value};

let root = Element::from_window(hwnd);
let result: Value = root.call_function("namespace.name", &make_args!(1,"2",3));
```

Calling Rust from script can be implemented as following:
```rust
struct Handler;

impl Handler {
  fn calc_sum(&self, a: i32, b: i32) -> i32 {
    a + b
  }
}

impl sciter::EventHandler for Handler {
  dispatch_script_call! {
    fn calc_sum(i32, i32);
  }
}
```

And we can access this function from script:
```js
// `view` represents window where script is runnung.
// `stdout` stream is a standard output stream (shell or debugger console, for example)

stdout.printf("2 + 3 = %d\n", view.calc_sum(2, 3));
```

_Check [rust-sciter/examples](https://github.com/sciter-sdk/rust-sciter/tree/master/examples) folder for more complex usage_.


## [Library documentation](https://docs.rs/sciter-rs/).


## What is supported right now:

* [x] [sciter::window](https://github.com/c-smile/sciter-sdk/blob/master/include/sciter-x-window.hpp) which brings together window creation, host and event handlers.
* [x] [sciter::host](https://github.com/c-smile/sciter-sdk/blob/master/include/sciter-x-host-callback.h) with basic event handling, needs to be redesigned.
* [x] [sciter::event_handler](https://github.com/c-smile/sciter-sdk/blob/master/include/sciter-x-behavior.h) with event handling and auto dispatching script calls to native code.
* [x] [sciter::dom](https://github.com/c-smile/sciter-sdk/blob/master/include/sciter-x-dom.hpp) for HTML DOM access and manipulation methods.
* [x] [sciter::value](https://github.com/c-smile/sciter-sdk/blob/master/include/value.hpp) Rust wrapper.
* [x] [sciter::behavior_factory](https://github.com/c-smile/sciter-sdk/blob/master/include/sciter-x-behavior.h) - global factory for native behaviors.
* [x] [sciter::graphics](https://github.com/c-smile/sciter-sdk/blob/master/include/sciter-x-graphics.hpp) - platform independent graphics native interface (can be used in native behaviors).
* [x] [sciter::request](https://github.com/c-smile/sciter-sdk/blob/master/include/sciter-x-request.hpp) - resource request object, used for custom resource downloading and handling.
* [x] [sciter::video](https://github.com/c-smile/sciter-sdk/blob/master/include/sciter-x-video-api.h) - custom video rendering.
* [x] [sciter::archive](https://github.com/c-smile/sciter-sdk/blob/master/include/sciter-x-host-callback.h) - Sciter's compressed archive produced by `sdk/bin/packfolder` tool.
* [x] [sciter::msg](https://github.com/c-smile/sciter-sdk/blob/master/include/sciter-x-msg.h) - backend-independent input event processing.
* [x] [sciter::om](https://github.com/c-smile/sciter-sdk/blob/master/include/sciter-om.h) - Sciter Object Model, extending Sciter by native code.
* [x] [NSE](https://sciter.com/include-library-name-native-extensions/) - native Sciter extensions.

### Platforms:

* [x] Windows
* [x] OSX
* [x] Linux
* [x] Raspberry Pi


## License

Bindings library licensed under [MIT license](https://opensource.org/licenses/MIT). Sciter Engine has the [own license terms](https://sciter.com/prices/) and [end used license agreement](https://github.com/c-smile/sciter-sdk/blob/master/license.htm) for SDK usage.
