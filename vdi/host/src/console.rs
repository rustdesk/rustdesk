use hbb_common::{log, tokio, ResultType};
use image::GenericImage;
use qemu_display::{Console, ConsoleListenerHandler, MouseButton};
use std::{collections::HashSet, sync::Arc, time};
pub use tokio::sync::{mpsc, Mutex};

#[derive(Debug)]
pub enum Event {
    ConsoleUpdate((i32, i32, i32, i32)),
    Disconnected,
}

const PIXMAN_X8R8G8B8: u32 = 0x20020888;
pub type BgraImage = image::ImageBuffer<image::Rgba<u8>, Vec<u8>>;
#[derive(Debug)]
pub struct ConsoleListener {
    pub image: Arc<Mutex<BgraImage>>,
    pub tx: mpsc::UnboundedSender<Event>,
}

#[async_trait::async_trait]
impl ConsoleListenerHandler for ConsoleListener {
    async fn scanout(&mut self, s: qemu_display::Scanout) {
        *self.image.lock().await = image_from_vec(s.format, s.width, s.height, s.stride, s.data);
    }

    async fn update(&mut self, u: qemu_display::Update) {
        let update = image_from_vec(u.format, u.w as _, u.h as _, u.stride, u.data);
        let mut image = self.image.lock().await;
        if (u.x, u.y) == (0, 0) && update.dimensions() == image.dimensions() {
            *image = update;
        } else {
            image.copy_from(&update, u.x as _, u.y as _).unwrap();
        }
        self.tx
            .send(Event::ConsoleUpdate((u.x, u.y, u.w, u.h)))
            .ok();
    }

    async fn scanout_dmabuf(&mut self, _scanout: qemu_display::ScanoutDMABUF) {
        unimplemented!()
    }

    async fn update_dmabuf(&mut self, _update: qemu_display::UpdateDMABUF) {
        unimplemented!()
    }

    async fn mouse_set(&mut self, set: qemu_display::MouseSet) {
        dbg!(set);
    }

    async fn cursor_define(&mut self, cursor: qemu_display::Cursor) {
        dbg!(cursor);
    }

    fn disconnected(&mut self) {
        dbg!();
    }
}

#[derive(derivative::Derivative)]
#[derivative(Debug)]
pub struct Client {
    #[derivative(Debug = "ignore")]
    console: Arc<Mutex<Console>>,
    last_update: Option<time::Instant>,
    has_update: bool,
    req_update: bool,
    last_buttons: HashSet<MouseButton>,
    dimensions: (u16, u16),
    image: Arc<Mutex<BgraImage>>,
}

impl Client {
    pub fn new(console: Arc<Mutex<Console>>, image: Arc<Mutex<BgraImage>>) -> Self {
        Self {
            console,
            image,
            last_update: None,
            has_update: false,
            req_update: false,
            last_buttons: HashSet::new(),
            dimensions: (0, 0),
        }
    }

    pub fn update_pending(&self) -> bool {
        self.has_update && self.req_update
    }

    pub async fn key_event(&self, qnum: u32, down: bool) -> ResultType<()> {
        let console = self.console.lock().await;
        if down {
            console.keyboard.press(qnum).await?;
        } else {
            console.keyboard.release(qnum).await?;
        }
        Ok(())
    }

    pub async fn desktop_resize(&mut self) -> ResultType<()> {
        let image = self.image.lock().await;
        let (width, height) = (image.width() as _, image.height() as _);
        if (width, height) == self.dimensions {
            return Ok(());
        }
        self.dimensions = (width, height);
        Ok(())
    }

    pub async fn send_framebuffer_update(&mut self) -> ResultType<()> {
        self.desktop_resize().await?;
        if self.has_update && self.req_update {
            if let Some(last_update) = self.last_update {
                if last_update.elapsed().as_millis() < 10 {
                    log::info!("TODO: <10ms, could delay update..")
                }
            }
            // self.server.send_framebuffer_update(&self.vnc_server)?;
            self.last_update = Some(time::Instant::now());
            self.has_update = false;
            self.req_update = false;
        }
        Ok(())
    }

    pub async fn handle_event(&mut self, event: Option<Event>) -> ResultType<bool> {
        match event {
            Some(Event::ConsoleUpdate(_)) => {
                self.has_update = true;
            }
            Some(Event::Disconnected) => {
                return Ok(false);
            }
            None => {
                self.send_framebuffer_update().await?;
            }
        }

        Ok(true)
    }
}

fn image_from_vec(format: u32, width: u32, height: u32, stride: u32, data: Vec<u8>) -> BgraImage {
    if format != PIXMAN_X8R8G8B8 {
        todo!("unhandled pixman format: {}", format)
    }
    if cfg!(target_endian = "big") {
        todo!("pixman/image in big endian")
    }
    let layout = image::flat::SampleLayout {
        channels: 4,
        channel_stride: 1,
        width,
        width_stride: 4,
        height,
        height_stride: stride as _,
    };
    let samples = image::flat::FlatSamples {
        samples: data,
        layout,
        color_hint: None,
    };
    samples
        .try_into_buffer::<image::Rgba<u8>>()
        .or_else::<&str, _>(|(_err, samples)| {
            let view = samples.as_view::<image::Rgba<u8>>().unwrap();
            let mut img = BgraImage::new(width, height);
            img.copy_from(&view, 0, 0).unwrap();
            Ok(img)
        })
        .unwrap()
}

fn button_mask_to_set(mask: u8) -> HashSet<MouseButton> {
    let mut set = HashSet::new();
    if mask & 0b0000_0001 != 0 {
        set.insert(MouseButton::Left);
    }
    if mask & 0b0000_0010 != 0 {
        set.insert(MouseButton::Middle);
    }
    if mask & 0b0000_0100 != 0 {
        set.insert(MouseButton::Right);
    }
    if mask & 0b0000_1000 != 0 {
        set.insert(MouseButton::WheelUp);
    }
    if mask & 0b0001_0000 != 0 {
        set.insert(MouseButton::WheelDown);
    }
    set
}
