[![cargo version](https://img.shields.io/crates/v/nokhwa-core.svg)](https://crates.io/crates/nokhwa-core) 
[![docs.rs version](https://img.shields.io/docsrs/nokhwa-core)](https://docs.rs/nokhwa/latest/nokhwa-core/)
# nokhwa-core
This crate contains core type definitions for `nokhwa`. This is seperated so other crates may use it as well.

Inside there are standard definitions (`Resolution`, `CameraInfo`, `CameraIndex`, `CameraFormat`, etc.), and 
there are decoders for NV12, YUY2/YUYV, MJPEG, GRAY, and RGB24, with a flexible trait based system for you to add your
own decoders. 