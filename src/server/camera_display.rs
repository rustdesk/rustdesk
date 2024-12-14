use std::{io, sync::{Arc, Mutex}};
use nokhwa::{
    pixel_format::RgbAFormat, query, utils::{
        ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType
    }, Camera 
};
use scrap::{Frame, PixelBuffer, TraitCapturer};
use super::*;



lazy_static::lazy_static! {
    pub static ref SYNC_CAMERA_DISPLAYS: Arc<Mutex<Vec<DisplayInfo>>> = Arc::new(Mutex::new(Vec::new()));
}
pub struct Cameras {}
impl Cameras {
    pub fn all(displays:&[DisplayInfo]) -> ResultType<Vec<DisplayInfo>> {        
        match query(ApiBackend::Auto) {
            Ok(cameras) => {
                let Some(last_display) = displays.last().cloned()
                else{
                    bail!("No display found")
                };
                let mut x = last_display.x+last_display.width;
                let y= last_display.y;
                let mut camera_displays = SYNC_CAMERA_DISPLAYS.lock().unwrap();
                camera_displays.clear();
                for info in &cameras {
                    let camera = Self::create_camera(info.index())?;
                    let resolution = camera.resolution();
                    let (width, height) = (resolution.width() as i32, resolution.height() as i32);
                    camera_displays.push(DisplayInfo {
                        x,
                        y,
                        name: info.human_name().clone(),
                        width,
                        height,
                        online: true,
                        cursor_embedded: false,
                        scale:1.0,
                        ..Default::default()
                    });
                    x += width;
                }
                Ok(displays.iter().chain(camera_displays.iter()).cloned().collect::<Vec<_>>())
            },
            Err(e) =>bail!("Query cameras error: {}", e)
        }
    }
    fn create_camera(index: &CameraIndex) -> ResultType<Camera> {
        let result = Camera::new(index.clone(), RequestedFormat::new::<RgbAFormat>(RequestedFormatType::AbsoluteHighestResolution));
        match result {
            Ok(camera)=> Ok(camera),
            Err(e) =>  bail!("create camera error:  {}", e),
        }
    }
    pub fn get_sync_cameras()->Vec<DisplayInfo>{
        SYNC_CAMERA_DISPLAYS.lock().unwrap().clone()
    }
    pub fn get_capturer(current : usize)->ResultType<Box<dyn TraitCapturer>>{
        Ok(Box::new(CameraCapturer::new(current)?))
    }
}


pub struct CameraCapturer {
    camera: Camera,
    data: Vec<u8>,
}
impl CameraCapturer {
    fn new(current: usize) -> ResultType<Self>  {
        let index = CameraIndex::Index(current as u32);
        let camera = Cameras::create_camera(&index)?;
        Ok(CameraCapturer {
            camera,
            data: Vec::new(),
        })
    }
}
impl TraitCapturer for CameraCapturer {
    fn frame<'a>(&'a mut self, _timeout: std::time::Duration) -> std::io::Result<scrap::Frame<'a>> {
        match self.camera.frame() {
            Ok(buffer) => {
                match buffer.decode_image::<RgbAFormat>() {
                    Ok(mut decoded) => {
                        for chunk in decoded.chunks_mut(4) {
                            chunk.swap(0, 2);
                        }
                        self.data = decoded.as_raw().to_vec();
                        Ok(Frame::PixelBuffer(PixelBuffer::new(
                            &self.data,
                            decoded.width() as usize,
                            decoded.height() as usize,
                        )))
                    },
                    Err(e) => Err(io::Error::new(io::ErrorKind::Other, format!("Camera frame decode error: {}", e))),
                }
            }
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, format!("Camera frame error: {}", e))),
        }

    }
    fn is_gdi(&self) -> bool {
        false
    }
    fn set_gdi(&mut self) -> bool {
        false
    }
}
