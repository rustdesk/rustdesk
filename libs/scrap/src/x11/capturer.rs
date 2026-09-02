use super::ffi::*;
use super::Display;
use crate::Pixfmt;
use hbb_common::libc;
use std::{io, ptr, slice};

pub struct Capturer {
    display: Display,
    shmid: i32,
    xcbid: u32,
    buffer: *const u8,

    size: usize,
    saved_raw_data: Vec<u8>, // for faster compare and copy
    converted: Vec<u8>,
}

impl Capturer {
    pub fn new(display: Display) -> io::Result<Capturer> {
        // Calculate dimensions.

        let pixel_width = display.pixfmt().bytes_per_pixel();
        let rect = display.rect();
        let size = (rect.w as usize) * (rect.h as usize) * pixel_width;

        // Create a shared memory segment.

        let shmid = unsafe {
            libc::shmget(
                libc::IPC_PRIVATE,
                size,
                // Everyone can do anything.
                libc::IPC_CREAT | 0o777,
            )
        };

        if shmid == -1 {
            return Err(io::Error::last_os_error());
        }

        // Attach the segment to a readable address.

        let buffer = unsafe { libc::shmat(shmid, ptr::null(), libc::SHM_RDONLY) } as *mut u8;

        if buffer as isize == -1 {
            return Err(io::Error::last_os_error());
        }

        // Attach the segment to XCB.

        let server = display.server().raw();
        let xcbid = unsafe { xcb_generate_id(server) };
        unsafe {
            xcb_shm_attach(
                server,
                xcbid,
                shmid as u32,
                0, // False, i.e. not read-only.
            );
        }

        let c = Capturer {
            display,
            shmid,
            xcbid,
            buffer,
            size,
            saved_raw_data: Vec::new(),
            converted: Vec::new(),
        };
        Ok(c)
    }

    pub fn display(&self) -> &Display {
        &self.display
    }

    fn get_image(&self) {
        let rect = self.display.rect();
        unsafe {
            let request = xcb_shm_get_image_unchecked(
                self.display.server().raw(),
                self.display.root(),
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                !0,
                XCB_IMAGE_FORMAT_Z_PIXMAP,
                self.xcbid,
                0,
            );
            let response =
                xcb_shm_get_image_reply(self.display.server().raw(), request, ptr::null_mut());
            libc::free(response as *mut _);
        }
    }

    /// Format of the frames `frame` returns; a depth-30 root is handed out as BGRA.
    pub fn pixfmt(&self) -> Pixfmt {
        match self.display.pixfmt() {
            Pixfmt::AR30 => Pixfmt::BGRA,
            pixfmt => pixfmt,
        }
    }

    pub fn frame<'b>(&'b mut self) -> std::io::Result<&'b [u8]> {
        self.get_image();
        let result = unsafe { slice::from_raw_parts(self.buffer, self.size) };
        crate::would_block_if_equal(&mut self.saved_raw_data, result)?;
        if self.display.pixfmt() == Pixfmt::AR30 {
            return self.ar30_to_bgra(result);
        }
        Ok(result)
    }

    fn ar30_to_bgra(&mut self, ar30: &[u8]) -> io::Result<&[u8]> {
        let rect = self.display.rect();
        ar30_to_bgra(ar30, rect.w as _, rect.h as _, &mut self.converted)?;
        Ok(&self.converted)
    }
}

/// xRGB2101010 (libyuv's AR30) to BGRA. The top two bits are padding, not
/// alpha, so the output alpha is forced opaque; libyuv has no XR30 conversion
/// and would expand them to 0/85/170/255.
fn ar30_to_bgra(ar30: &[u8], width: usize, height: usize, bgra: &mut Vec<u8>) -> io::Result<()> {
    let stride = width * 4;
    bgra.resize(stride * height, 0);
    let ret = unsafe {
        crate::common::AR30ToARGB(
            ar30.as_ptr(),
            stride as _,
            bgra.as_mut_ptr(),
            stride as _,
            width as _,
            height as _,
        )
    };
    if ret != 0 {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("AR30ToARGB failed: {ret}"),
        ));
    }
    for px in bgra.chunks_exact_mut(4) {
        px[3] = 0xff;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn ar30_to_bgra_drops_padding_bits() {
        // (padding, r, g, b) packed as X11 xRGB2101010, little endian.
        let px =
            |x: u32, r: u32, g: u32, b: u32| ((x << 30) | (r << 20) | (g << 10) | b).to_le_bytes();
        let src = [
            px(0, 1023, 0, 0),
            px(1, 0, 1023, 0),
            px(2, 0, 0, 1023),
            px(3, 512, 256, 128),
        ]
        .concat();
        let mut dst = Vec::new();
        super::ar30_to_bgra(&src, 4, 1, &mut dst).unwrap();
        assert_eq!(
            dst,
            [0, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 32, 64, 128, 255]
        );
    }
}

impl Drop for Capturer {
    fn drop(&mut self) {
        unsafe {
            // Detach segment from XCB.
            xcb_shm_detach(self.display.server().raw(), self.xcbid);
            // Detach segment from our space.
            libc::shmdt(self.buffer as *mut _);
            // Destroy the shared memory segment.
            libc::shmctl(self.shmid, libc::IPC_RMID, ptr::null_mut());
        }
    }
}
