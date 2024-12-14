use nokhwa::{
    pixel_format::RgbAFormat, query, utils::{
        ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType
    }, Camera 
};
use scrap::TraitCapturer;
use super::*;


lazy_static::lazy_static! {
    pub static ref SYNC_CAMERAS: Arc<Mutex<Vec<DisplayInfo>>> = Arc::new(Mutex::new(Vec::new()));
}
pub struct Cameras {}
impl Cameras {
    pub fn all(displays:&Vec<DisplayInfo>) -> Vec<DisplayInfo> {
        let last_display = displays.last().cloned().unwrap();
        let cameras = query(ApiBackend::Auto).unwrap();
        let (width,height) = (last_display.width ,last_display.height);
        let mut x = last_display.x;
        let y= last_display.y;
        let mut infos = SYNC_CAMERAS.lock().unwrap();
        *infos = cameras.iter()
        .map(|camera| {
            x += width;
            DisplayInfo {
                x,
                y,
                name: camera.human_name().clone(),
                width,
                height,
                online: true,
                cursor_embedded: true,
                scale:1.0,
                ..Default::default()
            }
        }).collect::<Vec<DisplayInfo>>();
        displays.iter().chain(infos.iter()).cloned().collect::<Vec<_>>()
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
                            bgra_image.width() as usize,
                            bgra_image.height() as usize,
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
