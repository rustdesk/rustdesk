use clap::Parser;
use hbb_common::{anyhow::Context, log, tokio, ResultType};
use qemu_display::{Console, VMProxy};
use std::{
    borrow::Borrow,
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
};

use crate::console::*;

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
struct Server {
    vm_name: String,
    rx: mpsc::UnboundedReceiver<Event>,
    tx: mpsc::UnboundedSender<Event>,
    image: Arc<Mutex<BgraImage>>,
    console: Arc<Mutex<Console>>,
}

impl Server {
    async fn new(vm_name: String, console: Console) -> ResultType<Server> {
        let width = console.width().await?;
        let height = console.height().await?;
        let image = BgraImage::new(width as _, height as _);
        let (tx, rx) = mpsc::unbounded_channel();
        Ok(Self {
            vm_name,
            rx,
            image: Arc::new(Mutex::new(image)),
            tx,
            console: Arc::new(Mutex::new(console)),
        })
    }

    async fn stop_console(&self) -> ResultType<()> {
        self.console.lock().await.unregister_listener();
        Ok(())
    }

    async fn run_console(&self) -> ResultType<()> {
        self.console
            .lock()
            .await
            .register_listener(ConsoleListener {
                image: self.image.clone(),
                tx: self.tx.clone(),
            })
            .await?;
        Ok(())
    }

    async fn dimensions(&self) -> (u16, u16) {
        let image = self.image.lock().await;
        (image.width() as u16, image.height() as u16)
    }

    async fn handle_connection(&mut self, stream: TcpStream) -> ResultType<()> {
        let (width, height) = self.dimensions().await;

        let tx = self.tx.clone();
        let _client_thread = thread::spawn(move || loop {});

        let mut client = Client::new(self.console.clone(), self.image.clone());
        self.run_console().await?;
        loop {
            let ev = if client.update_pending() {
                match self.rx.try_recv() {
                    Ok(e) => Some(e),
                    Err(mpsc::error::TryRecvError::Empty) => None,
                    Err(e) => {
                        return Err(e.into());
                    }
                }
            } else {
                Some(
                    self.rx
                        .recv()
                        .await
                        .context("Channel closed unexpectedly")?,
                )
            };
            if !client.handle_event(ev).await? {
                break;
            }
        }

        self.stop_console().await?;
        Ok(())
    }
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
    let mut server = Server::new(format!("qemu-rustdesk ({})", vm_name), console).await?;
    for stream in listener.incoming() {
        let stream = stream?;
        if let Err(err) = server.handle_connection(stream).await {
            log::error!("Connection closed: {err}");
        }
    }

    Ok(())
}
