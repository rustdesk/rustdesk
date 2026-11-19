/*
 * Copyright 2022 l1npengtul <l1npengtul@protonmail.com> / The Nokhwa Contributors
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
use crate::{
    error::NokhwaError,
    pixel_format::FormatDecoder,
    types::{FrameFormat, Resolution},
};
use bytes::Bytes;
use image::ImageBuffer;
#[cfg(feature = "opencv-mat")]
use opencv::{boxed_ref::BoxedRef, core::Mat};

/// A buffer returned by a camera to accommodate custom decoding.
/// Contains information of Resolution, the buffer's [`FrameFormat`], and the buffer.
///
/// Note that decoding on the main thread **will** decrease your performance and lead to dropped frames.
#[derive(Clone, Debug, Hash, PartialOrd, PartialEq, Eq)]
pub struct Buffer {
    resolution: Resolution,
    buffer: Bytes,
    source_frame_format: FrameFormat,
}

impl Buffer {
    /// Creates a new buffer with a [`&[u8]`].
    #[must_use]
    #[inline]
    pub fn new(res: Resolution, buf: &[u8], source_frame_format: FrameFormat) -> Self {
        Self {
            resolution: res,
            buffer: Bytes::copy_from_slice(buf),
            source_frame_format,
        }
    }

    /// Get the [`Resolution`] of this buffer.
    #[must_use]
    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    /// Get the data of this buffer.
    #[must_use]
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    /// Get a owned version of this buffer.
    #[must_use]
    pub fn buffer_bytes(&self) -> Bytes {
        self.buffer.clone()
    }

    /// Get the [`FrameFormat`] of this buffer.
    #[must_use]
    pub fn source_frame_format(&self) -> FrameFormat {
        self.source_frame_format
    }

    /// Decodes a image with allocation using the provided [`FormatDecoder`].
    /// # Errors
    /// Will error when the decoding fails.
    #[inline]
    pub fn decode_image<F: FormatDecoder>(
        &self,
    ) -> Result<ImageBuffer<F::Output, Vec<u8>>, NokhwaError> {
        let new_data = F::write_output(self.source_frame_format, self.resolution, &self.buffer)?;
        let image =
            ImageBuffer::from_raw(self.resolution.width_x, self.resolution.height_y, new_data)
                .ok_or(NokhwaError::ProcessFrameError {
                    src: self.source_frame_format,
                    destination: stringify!(F).to_string(),
                    error: "Failed to create buffer".to_string(),
                })?;
        Ok(image)
    }

    /// Decodes a image with allocation using the provided [`FormatDecoder`] into a `buffer`.
    /// # Errors
    /// Will error when the decoding fails, or the provided buffer is too small.
    #[inline]
    pub fn decode_image_to_buffer<F: FormatDecoder>(
        &self,
        buffer: &mut [u8],
    ) -> Result<(), NokhwaError> {
        F::write_output_buffer(
            self.source_frame_format,
            self.resolution,
            &self.buffer,
            buffer,
        )
    }

    /// Decodes an image with allocation using the provided [`FormatDecoder`] into a [`Mat`](https://docs.rs/opencv/latest/opencv/core/struct.Mat.html).
    ///
    /// Note that this does a clone when creating the buffer, to decouple the lifetime of the internal data to the temporary Buffer. If you want to avoid this, please see [`decode_opencv_mat`](Self::decode_opencv_mat).
    ///
    /// This is **NOT** coherent when the input data is not Gray8, GrayAlpha87, RGB8, or RGBA8
    /// # Errors
    /// Will error when the decoding fails, or `OpenCV` failed to create/copy the [`Mat`](https://docs.rs/opencv/latest/opencv/core/struct.Mat.html).
    /// # Safety
    /// This function uses `unsafe` in order to create the [`Mat`](https://docs.rs/opencv/latest/opencv/core/struct.Mat.html). Please see [`Mat::new_rows_cols_with_data`](https://docs.rs/opencv/latest/opencv/core/struct.Mat.html#method.new_rows_cols_with_data) for more.
    ///
    /// Most notably, the `data` **must** stay in scope for the duration of the [`Mat`](https://docs.rs/opencv/latest/opencv/core/struct.Mat.html) or bad, ***bad*** things happen.
    #[cfg(feature = "opencv-mat")]
    #[cfg_attr(feature = "docs-features", doc(cfg(feature = "opencv-mat")))]
    pub fn decode_opencv_mat<F: FormatDecoder>(
        &mut self,
    ) -> Result<BoxedRef<Mat>, NokhwaError> {
        use crate::buffer::channel_defs::make_mat;

        make_mat::<F>(self.resolution, self.buffer())
    }

    /// Decodes an image with allocation using the provided [`FormatDecoder`] into a [`Mat`](https://docs.rs/opencv/latest/opencv/core/struct.Mat.html).
    ///
    /// This is **NOT** coherent when the input data is not Gray8, GrayAlpha87, RGB8, or RGBA8
    /// # Errors
    /// Will error when the decoding fails, or `OpenCV` failed to create/copy the [`Mat`](https://docs.rs/opencv/latest/opencv/core/struct.Mat.html).
    #[cfg(feature = "opencv-mat")]
    #[cfg_attr(feature = "docs-features", doc(cfg(feature = "opencv-mat")))]
    #[allow(clippy::cast_possible_wrap)]
    pub fn decode_into_opencv_mat<F: FormatDecoder>(
        &mut self,
        dst: &mut Mat,
    ) -> Result<(), NokhwaError> {
        use image::Pixel;
        use opencv::core::{
            Mat, MatTraitConst, MatTraitManual, Scalar, CV_8UC1, CV_8UC2, CV_8UC3, CV_8UC4,
        };
        use bytes::Buf;

        let array_type = match F::Output::CHANNEL_COUNT {
            1 => CV_8UC1,
            2 => CV_8UC2,
            3 => CV_8UC3,
            4 => CV_8UC4,
            _ => {
                return Err(NokhwaError::ProcessFrameError {
                    src: FrameFormat::RAWRGB,
                    destination: "OpenCV Mat".to_string(),
                    error: "Invalid Decoder FormatDecoder Channel Count".to_string(),
                })
            }
        };

        // If destination does not exist, create a new matrix.
        if dst.empty() {
            *dst = Mat::new_rows_cols_with_default(
                self.resolution.height_y as i32,
                self.resolution.width_x as i32,
                array_type,
                Scalar::default(),
            )
            .map_err(|why| NokhwaError::ProcessFrameError {
                src: FrameFormat::RAWRGB,
                destination: "OpenCV Mat".to_string(),
                error: why.to_string(),
            })?;
        } else {
            if dst.typ() != array_type {
                return Err(NokhwaError::ProcessFrameError {
                    src: FrameFormat::RAWRGB,
                    destination: "OpenCV Mat".to_string(),
                    error: "Invalid Matrix Channel Count".to_string(),
                });
            }

            if dst.rows() != self.resolution.height_y as _
                || dst.cols() != self.resolution.width_x as _
            {
                return Err(NokhwaError::ProcessFrameError {
                    src: FrameFormat::RAWRGB,
                    destination: "OpenCV Mat".to_string(),
                    error: "Invalid Matrix Dimensions".to_string(),
                });
            }
        }

        let mut bytes = match dst.data_bytes_mut() {
            Ok(bytes) => bytes,
            Err(_e) => {
                return Err(NokhwaError::ProcessFrameError {
                    src: FrameFormat::RAWRGB,
                    destination: "OpenCV Mat".to_string(),
                    error: "Matrix Must Be Continuous".to_string(),
                })
            }
        };

        let mut buffer = self.buffer.as_ref();
        if bytes.len() != buffer.len() {
            return Err(NokhwaError::ProcessFrameError {
                src: FrameFormat::RAWRGB,
                destination: "OpenCV Mat".to_string(),
                error: "Matrix Buffer Size Mismatch".to_string(),
            });
        }

        buffer.copy_to_slice(&mut bytes);

        Ok(())
    }
}

/// Channel definitions and utilities for making Mat for OpenCV
///
/// You (probably) shouldn't use this.
#[cfg(feature = "opencv-mat")]
pub mod channel_defs {
    use bytemuck::{cast_slice, Pod, Zeroable};
    use image::Pixel;
    use crate::error::NokhwaError;
    use crate::pixel_format::FormatDecoder;
    use crate::types::{FrameFormat, Resolution};

    #[cfg(feature = "opencv-mat")]
    #[cfg_attr(feature = "docs-features", doc(cfg(feature = "opencv-mat")))]
    pub(crate) fn make_mat<F>(resolution: Resolution, data: &[u8]) -> Result<opencv::boxed_ref::BoxedRef<opencv::core::Mat>, NokhwaError> where F: FormatDecoder {
        use crate::buffer::channel_defs::*;
        use opencv::core::Mat;

        let mat = match F::Output::CHANNEL_COUNT {
            1 => Mat::new_rows_cols_with_data::<G8>(resolution.width() as i32, resolution.height() as i32, cast_slice(data)),
            2 => Mat::new_rows_cols_with_data::<GA8>(resolution.width() as i32, resolution.height() as i32, cast_slice(data)),
            3 => Mat::new_rows_cols_with_data::<RGB8>(resolution.width() as i32, resolution.height() as i32, cast_slice(data)),
            4 => Mat::new_rows_cols_with_data::<RGBA8>(resolution.width() as i32, resolution.height() as i32, cast_slice(data)),
            _ => {
                return Err(NokhwaError::ProcessFrameError {
                    src: FrameFormat::RAWRGB,
                    destination: "OpenCV Mat".to_string(),
                    error: "Invalid Decoder FormatDecoder Channel Count".to_string(),
                })
            }
        };

        match mat {
            Ok(m) => Ok(m),
            Err(why) => Err(NokhwaError::ProcessFrameError {
                src: FrameFormat::RAWRGB,
                destination: "OpenCV Mat".to_string(),
                error: why.to_string(),
            })
        }
    }


    /// Three u8
    #[repr(transparent)]
    #[derive(Copy, Clone, Debug)]
    pub struct RGB8 {
        pub data: [u8; 3]
    }

    unsafe impl opencv::core::DataType for RGB8 {
        fn opencv_depth() -> i32 {
            1
        }

        fn opencv_channels() -> i32 {
            3
        }
    }

    unsafe impl Pod for RGB8 {}

    unsafe impl Zeroable for RGB8 {}

    /// Two u8
    #[repr(transparent)]
    #[derive(Copy, Clone, Debug)]
    pub struct GA8 {
        pub data: [u8; 2],
    }

    unsafe impl opencv::core::DataType for GA8 {
        fn opencv_depth() -> i32 {
            1
        }

        fn opencv_channels() -> i32 {
            2
        }
    }

    unsafe impl Zeroable for GA8 {}

    unsafe impl Pod for GA8 {}

    /// One u8
    #[derive(Copy, Clone, Debug)]
    pub struct G8 {
        pub data: u8,
    }

    unsafe impl opencv::core::DataType for G8 {
        fn opencv_depth() -> i32 {
            1
        }

        fn opencv_channels() -> i32 {
            1
        }
    }

    unsafe impl Zeroable for G8 {}

    unsafe impl Pod for G8 {}

    /// Four u8
    #[repr(transparent)]
    #[derive(Copy, Clone, Debug)]
    pub struct RGBA8 {
        pub data: [u8; 4]
    }

    unsafe impl opencv::core::DataType for RGBA8 {
        fn opencv_depth() -> i32 {
            1
        }

        fn opencv_channels() -> i32 {
            4
        }
    }

    unsafe impl Zeroable for RGBA8 {}

    unsafe impl Pod for RGBA8 {}
}