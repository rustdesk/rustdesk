use nokhwa::{
    pixel_format::RgbAFormat, query, utils::{
        ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType
    }, Camera 
};
use scrap::{Display, TraitCapturer};
use std::convert::TryInto;

use super::*;


lazy_static::lazy_static! {
    pub static ref SYNC_CAMERAS: Arc<Mutex<Vec<DisplayInfo>>> = Arc::new(Mutex::new(Vec::new()));
}
pub struct Cameras {}
impl Cameras {
    pub fn all() -> Vec<DisplayInfo> {
        match Display::all(){
            Ok(displays) => {
                let last_display = displays.last().unwrap();
                let cameras = query(ApiBackend::Auto).unwrap();
                let mut infos = SYNC_CAMERAS.lock().unwrap();
                let (width,height) = (last_display.width() as i32,last_display.height() as i32);
                let mut origin_left = last_display.origin().0;
                *infos = cameras.iter()
                .map(|camera| {
                    origin_left += width as i32;
                    DisplayInfo {
                        x: origin_left,
                        y: 0,
                        name: camera.human_name().clone(),
                        width,
                        height,
                        online: true,
                        cursor_embedded: true,
                        scale:1.0,
                        ..Default::default()
                    }
                }).collect::<Vec<DisplayInfo>>();
                infos.clone()
            },
            Err(_) => todo!(),
        }
    }
    pub fn get_cameras()->Vec<DisplayInfo>{
        SYNC_CAMERAS.lock().unwrap().clone()
    }


    pub fn get_capturer(current : usize)->ResultType<Box<dyn TraitCapturer>>{
        Ok(Box::new(CameraCapturer::new(current)))
    }
}



use image::{RgbaImage, ImageBuffer};
trait ConvertToBgra {
    fn to_bgra(&self) -> RgbaImage;
}
impl ConvertToBgra for RgbaImage{
    fn to_bgra(&self) -> RgbaImage {
        let mut bgra_data = self.clone().into_raw();
        for chunk in bgra_data.chunks_mut(4) {
            chunk.swap(0, 2);
        }
        ImageBuffer::from_raw(self.width(), self.height(), bgra_data).unwrap()
    }
}

pub struct CameraCapturer {
    camera: Camera,
    data: Vec<u8>,
}
impl CameraCapturer {
    fn new(current_display: usize) -> Self  {
        let index = CameraIndex::Index(current_display.try_into().unwrap());
        let format: RequestedFormat<'_> = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::AbsoluteHighestResolution);
        let camera = Camera::new(index, format).unwrap();
        CameraCapturer {
            camera,
            data: Vec::new(),
        }
    }
}
impl TraitCapturer for CameraCapturer {
    fn frame<'a>(&'a mut self, _timeout: std::time::Duration) -> std::io::Result<scrap::Frame<'a>> {
        match self.camera.frame() {
            Ok(buffer) => {
                match buffer.decode_image::<RgbAFormat>() {
                    Ok(decoded) => {
                        let bgra_image = decoded.to_bgra();
                        self.data = bgra_image.as_raw().to_vec();
                        Ok(scrap::Frame::PixelBuffer(scrap::PixelBuffer::new(
                            self.data.as_mut_slice(),
                            bgra_image.width() as _,
                            bgra_image.height() as _,
                        )))
                    },
                    Err(e) => {
                        Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Camera frame decode error: {}", e))) 
                    },
                }
            }
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Camera frame error: {}", e))),
        }

    }
    fn is_gdi(&self) -> bool {
        false
    }
    fn set_gdi(&mut self) -> bool {
        false
    }
}
