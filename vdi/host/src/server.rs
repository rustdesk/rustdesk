use clap::Parser;
use hbb_common::{
    allow_err,
    anyhow::{bail, Context},
    log,
    message_proto::*,
    protobuf::Message as _,
    tokio,
    tokio::net::TcpListener,
    ResultType, Stream,
};
use qemu_display::{Console, VMProxy};
use std::{borrow::Borrow, sync::Arc};

use crate::connection::*;
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
    rx_console: mpsc::UnboundedReceiver<Event>,
    tx_console: mpsc::UnboundedSender<Event>,
    rx_conn: mpsc::UnboundedReceiver<Message>,
    tx_conn: mpsc::UnboundedSender<Message>,
    image: Arc<Mutex<BgraImage>>,
    console: Arc<Mutex<Console>>,
}

impl Server {
    async fn new(vm_name: String, console: Console) -> ResultType<Server> {
        let width = console.width().await?;
        let height = console.height().await?;
        let image = BgraImage::new(width as _, height as _);
        let (tx_console, rx_console) = mpsc::unbounded_channel();
        let (tx_conn, rx_conn) = mpsc::unbounded_channel();
        Ok(Self {
            vm_name,
            rx_console,
            tx_console,
            rx_conn,
            tx_conn,
            image: Arc::new(Mutex::new(image)),
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
                tx: self.tx_console.clone(),
            })
            .await?;
        Ok(())
    }

    async fn dimensions(&self) -> (u16, u16) {
        let image = self.image.lock().await;
        (image.width() as u16, image.height() as u16)
    }

    async fn handle_connection(&mut self, stream: Stream) -> ResultType<()> {
        let mut stream = stream;
        self.run_console().await?;
        let mut conn = Connection {
            tx: self.tx_conn.clone(),
        };

        loop {
            tokio::select! {
                Some(evt) = self.rx_console.recv() => {
                    match evt {
                        _ => {}
                    }
                }
                Some(msg) = self.rx_conn.recv() => {
                    allow_err!(stream.send(&msg).await);
                }
                res = stream.next() => {
                    if let Some(res) = res {
                        match res {
                            Err(err) => {
                                bail!(err);
                            }
                            Ok(bytes) => {
                                if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
                                    match conn.on_message(msg_in).await {
                                        Ok(false) => {
                                            break;
                                        }
                                        Err(err) => {
                                            log::error!("{err}");
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    } else {
                        bail!("Reset by the peer");
                    }
                }
            }
        }

        self.stop_console().await?;
        Ok(())
    }
}

#[tokio::main]
pub async fn run() -> ResultType<()> {
    let args = Cli::parse();

    let listener = TcpListener::bind::<std::net::SocketAddr>(args.address.into())
        .await
        .unwrap();
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
    loop {
        let (stream, addr) = listener.accept().await?;
        stream.set_nodelay(true).ok();
        let laddr = stream.local_addr()?;
        let stream = Stream::from(stream, laddr);
        if let Err(err) = server.handle_connection(stream).await {
            log::error!("Connection from {addr} closed: {err}");
        }
    }
}
