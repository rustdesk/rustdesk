Derived from https://github.com/quadrupleslap/scrap

# scrap

Scrap records your screen! At least it does if you're on Windows, macOS, or Linux.

## Usage

```toml
[dependencies]
scrap = "0.5"
```

Its API is as simple as it gets!

```rust
struct Display; /// A screen.
struct Frame; /// An array of the pixels that were on-screen.
struct Capturer; /// A recording instance.

impl Capturer {
    /// Begin recording.
    pub fn new(display: Display) -> io::Result<Capturer>;

    /// Try to get a frame.
    /// Returns WouldBlock if it's not ready yet.
    pub fn frame<'a>(&'a mut self) -> io::Result<Frame<'a>>;

    pub fn width(&self) -> usize;
    pub fn height(&self) -> usize;
}

impl Display {
    /// The primary screen.
    pub fn primary() -> io::Result<Display>;

    /// All the screens.
    pub fn all() -> io::Result<Vec<Display>>;

    pub fn width(&self) -> usize;
    pub fn height(&self) -> usize;
}

impl<'a> ops::Deref for Frame<'a> {
    /// A frame is just an array of bytes.
    type Target = [u8];
}
```

## The Frame Format

- The frame format is guaranteed to be **packed BGRA**.
- The width and height are guaranteed to remain constant.
- The stride might be greater than the width, and it may also vary between frames.

## System Requirements

OS      | Minimum Requirements
--------|---------------------
macOS   | macOS 10.8
Linux   | XCB + SHM + RandR
Windows | DirectX 11.1
