use clap::Parser;
use hbb_common::{
    anyhow::{anyhow, Context},
    log,
    message_proto::*,
    tokio, ResultType,
};
use image::GenericImage;
use qemu_display::{Console, ConsoleListenerHandler, MouseButton, VMProxy};
use std::{
    borrow::Borrow,
    collections::HashSet,
    error::Error,
    io,
    iter::FromIterator,
    net::{TcpListener, TcpStream},
    sync::{mpsc, Arc, Mutex},
    thread, time,
};

#[derive(Parser, Debug)]
pub struct SocketAddrArgs {
    /// IP address
    #[clap(short, long, default_value = "0.0.0.0")]
    address: std::net::IpAddr,
    /// IP port number
    #[clap(short, long, default_value = "21116")]
    port: u16,
}

impl From<SocketAddrArgs> for std::net::SocketAddr {
    fn from(args: SocketAddrArgs) -> Self {
        (args.address, args.port).into()
    }
}

#[derive(Parser, Debug)]
struct Cli {
    #[clap(flatten)]
    address: SocketAddrArgs,
    #[clap(short, long)]
    dbus_address: Option<String>,
}

#[derive(Debug)]
enum Event {
    ConsoleUpdate((i32, i32, i32, i32)),
    Disconnected,
}

const PIXMAN_X8R8G8B8: u32 = 0x20020888;
type BgraImage = image::ImageBuffer<image::Rgba<u8>, Vec<u8>>;

#[derive(derivative::Derivative)]
#[derivative(Debug)]
struct Client {
    #[derivative(Debug = "ignore")]
    server: Server,
    share: bool,
    last_update: Option<time::Instant>,
    has_update: bool,
    req_update: bool,
    last_buttons: HashSet<MouseButton>,
    dimensions: (u16, u16),
}

impl Client {
    fn new(server: Server, share: bool) -> Self {
        Self {
            server,
            share,
            last_update: None,
            has_update: false,
            req_update: false,
            last_buttons: HashSet::new(),
            dimensions: (0, 0),
        }
    }

    fn update_pending(&self) -> bool {
        self.has_update && self.req_update
    }

    async fn key_event(&self, qnum: u32, down: bool) -> ResultType<()> {
        let inner = self.server.inner.lock().unwrap();
        if down {
            inner.console.keyboard.press(qnum).await?;
        } else {
            inner.console.keyboard.release(qnum).await?;
        }
        Ok(())
    }

    fn desktop_resize(&mut self) -> ResultType<()> {
        let (width, height) = self.server.dimensions();
        if (width, height) == self.dimensions {
            return Ok(());
        }
        self.dimensions = (width, height);
        Ok(())
    }

    fn send_framebuffer_update(&mut self) -> ResultType<()> {
        self.desktop_resize()?;
        if self.has_update && self.req_update {
            if let Some(last_update) = self.last_update {
                if last_update.elapsed().as_millis() < 10 {
                    println!("TODO: <10ms, could delay update..")
                }
            }
            // self.server.send_framebuffer_update(&self.vnc_server)?;
            self.last_update = Some(time::Instant::now());
            self.has_update = false;
            self.req_update = false;
        }
        Ok(())
    }

    async fn handle_event(&mut self, event: Option<Event>) -> ResultType<bool> {
        match event {
            Some(Event::ConsoleUpdate(_)) => {
                self.has_update = true;
            }
            Some(Event::Disconnected) => {
                return Ok(false);
            }
            None => {
                self.send_framebuffer_update()?;
            }
        }

        Ok(true)
    }
}

#[derive(Debug)]
struct ConsoleListener {
    server: Server,
}

#[async_trait::async_trait]
impl ConsoleListenerHandler for ConsoleListener {
    async fn scanout(&mut self, s: qemu_display::Scanout) {
        let mut inner = self.server.inner.lock().unwrap();
        inner.image = image_from_vec(s.format, s.width, s.height, s.stride, s.data);
    }

    async fn update(&mut self, u: qemu_display::Update) {
        let mut inner = self.server.inner.lock().unwrap();
        let update = image_from_vec(u.format, u.w as _, u.h as _, u.stride, u.data);
        if (u.x, u.y) == (0, 0) && update.dimensions() == inner.image.dimensions() {
            inner.image = update;
        } else {
            inner.image.copy_from(&update, u.x as _, u.y as _).unwrap();
        }
        inner
            .tx
            .send(Event::ConsoleUpdate((u.x, u.y, u.w, u.h)))
            .unwrap();
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

#[derive(Debug)]
struct ServerInner {
    console: Console,
    image: BgraImage,
    tx: mpsc::Sender<Event>,
}

#[derive(Clone, Debug)]
struct Server {
    vm_name: String,
    rx: Arc<Mutex<mpsc::Receiver<Event>>>,
    inner: Arc<Mutex<ServerInner>>,
}

impl Server {
    async fn new(vm_name: String, console: Console) -> ResultType<Server> {
        let width = console.width().await?;
        let height = console.height().await?;
        let image = BgraImage::new(width as _, height as _);
        let (tx, rx) = mpsc::channel();
        Ok(Self {
            vm_name,
            rx: Arc::new(Mutex::new(rx)),
            inner: Arc::new(Mutex::new(ServerInner { console, image, tx })),
        })
    }

    fn stop_console(&self) -> ResultType<()> {
        let mut inner = self.inner.lock().unwrap();
        inner.console.unregister_listener();
        Ok(())
    }

    async fn run_console(&self) -> ResultType<()> {
        let inner = self.inner.lock().unwrap();
        inner
            .console
            .register_listener(ConsoleListener {
                server: self.clone(),
            })
            .await?;
        Ok(())
    }

    fn dimensions(&self) -> (u16, u16) {
        let inner = self.inner.lock().unwrap();
        (inner.image.width() as u16, inner.image.height() as u16)
    }

    async fn handle_connection(&self, stream: TcpStream) -> ResultType<()> {
        let (width, height) = self.dimensions();

        let tx = self.inner.lock().unwrap().tx.clone();
        let _client_thread = thread::spawn(move || loop {});

        let mut client = Client::new(self.clone(), true);
        self.run_console().await?;
        let rx = self.rx.lock().unwrap();
        loop {
            let ev = if client.update_pending() {
                match rx.try_recv() {
                    Ok(e) => Some(e),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(e) => {
                        return Err(e.into());
                    }
                }
            } else {
                Some(rx.recv()?)
            };
            if !client.handle_event(ev).await? {
                break;
            }
        }

        self.stop_console()?;
        Ok(())
    }
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

#[tokio::main]
pub async fn run() -> ResultType<()> {
    let args = Cli::parse();

    let listener = TcpListener::bind::<std::net::SocketAddr>(args.address.into()).unwrap();
    let dbus = if let Some(addr) = args.dbus_address {
        zbus::ConnectionBuilder::address(addr.borrow())?
            .build()
            .await
    } else {
        zbus::Connection::session().await
    }
    .context("Failed to connect to DBus")?;

    let vm_name = VMProxy::new(&dbus).await?.name().await?;
    let console = Console::new(&dbus.into(), 0)
        .await
        .context("Failed to get the console")?;
    let server = Server::new(format!("qemu-rustdesk ({})", vm_name), console).await?;
    for stream in listener.incoming() {
        let stream = stream?;
        let server = server.clone();
        if let Err(err) = server.handle_connection(stream).await {
            log::error!("Connection closed: {err}");
        }
    }

    Ok(())
}
