use nokhwa::{
    Camera,
    query, 
    pixel_format::RgbAFormat,
    utils::{
        ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType
    }, 
};
use scrap::{Display, TraitCapturer};
use hbb_common::anyhow::anyhow;

use super::*;


pub struct Cameras {

}
impl Cameras {
    pub fn all() -> Vec<DisplayInfo> {
        match Display::all(){
            Ok(displays) => {
                let first_display = displays.first().unwrap();
                let cameras = query(ApiBackend::Auto).unwrap();
                cameras.iter()
                .map(|camera| {
                    DisplayInfo {
                        name: camera.human_name().clone(),
                        width: first_display.width() as _,
                        height: first_display.height() as _,
                        online: true,
                        cursor_embedded: true,
                        scale:1.0,
                        ..Default::default()
                    }
                }).collect::<Vec<DisplayInfo>>()
            },
            Err(_) => todo!(),
        }
            
    }
    pub fn get_capturer(current : usize)->ResultType<Box<dyn TraitCapturer>>{
        Ok(Box::new(CameraCapturer::new(current)))
    }
}

use image::{Rgba, RgbaImage};
fn rgba_to_bgra(rgba_image: &RgbaImage) -> RgbaImage {
    let (width, height) = rgba_image.dimensions();
    let mut bgra_image = RgbaImage::new(width, height);
    for (x, y, pixel) in rgba_image.enumerate_pixels() {
        let rgba = pixel.0; 
        let bgra = Rgba([rgba[2], rgba[1], rgba[0], rgba[3]]);
        bgra_image.put_pixel(x, y, bgra);
    }
    bgra_image
}

pub struct CameraCapturer {
    camera: Camera,
    data: Vec<u8>,
}
impl CameraCapturer {
    fn new(current_display: usize) -> Self {
        let index = CameraIndex::Index(current_display.try_into().unwrap());
        let format: RequestedFormat<'_> = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::AbsoluteHighestResolution);
        CameraCapturer { 
            camera: Camera::new(index,format).unwrap(),
            data: Vec::<u8>::new(),
        }
    }
}
impl TraitCapturer for CameraCapturer {
    fn frame<'a>(&'a mut self, _timeout: std::time::Duration) -> std::io::Result<scrap::Frame<'a>> {
        match self.camera.frame() {
            Ok(buffer) => {
                match buffer.decode_image::<RgbAFormat>() {
                    Ok(decoded) => {
                        self.data = rgba_to_bgra(&decoded).as_raw().to_vec();
                        Ok(scrap::Frame::PixelBuffer(scrap::PixelBuffer::new(
                            &self.data,
                            decoded.width() as _,
                            decoded.height() as _,
                        )))
                    },
                    Err(err) => {
                        Err(std::io::Error::new(std::io::ErrorKind::Other, anyhow!(err.to_string()))) 
                    },
                }
            }
            Err(err) => {
                Err(std::io::Error::new(std::io::ErrorKind::Other, anyhow!(err.to_string()) ))    
            },
        }

    }

    fn is_gdi(&self) -> bool {
        false
    }

    fn set_gdi(&mut self) -> bool {
        false
    }
}
