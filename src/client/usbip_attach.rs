// Controller side of a `RemoteUsb` session: attaching a remote device means
// running the system `usbip attach`, which makes its own local TCP
// connection that has to land somewhere -- so we stand up a local listener
// that plays the role of the (otherwise unreachable) remote `usbipd`, and
// relay everything it sees through `UsbChannel` frames.
use crate::flutter::FlutterHandler;
use crate::ui_session_interface::Session;
use hbb_common::{
    bytes::Bytes, log,
    tokio::{
        self,
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::mpsc,
    },
};
use std::process::Command;

pub enum Inbound {
    Opened { success: bool, message: String },
    Data(Bytes),
    Closed,
}

pub async fn attach(session: Session<FlutterHandler>, bus_id: String) {
    let listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(err) => {
            log::error!("usb attach: failed to bind local listener: {}", err);
            return;
        }
    };
    let port = match listener.local_addr() {
        Ok(addr) => addr.port(),
        Err(err) => {
            log::error!("usb attach: failed to read local listener port: {}", err);
            return;
        }
    };

    let relay_session = session.clone();
    let relay_bus_id = bus_id.clone();
    tokio::spawn(async move {
        match listener.accept().await {
            Ok((socket, _)) => run_channel(relay_session, relay_bus_id, socket).await,
            Err(err) => log::error!("usb attach: accept failed: {}", err),
        }
    });

    let attach_result = tokio::task::spawn_blocking(move || {
        Command::new("usbip")
            .args(["-t", &port.to_string(), "attach", "-r", "127.0.0.1", "-b", &bus_id])
            .output()
    })
    .await;
    match attach_result {
        Ok(Ok(out)) if out.status.success() => {}
        Ok(Ok(out)) => log::error!(
            "usb attach: `usbip attach` failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ),
        Ok(Err(err)) => log::error!("usb attach: failed to run usbip: {}", err),
        Err(err) => log::error!("usb attach: spawn_blocking join error: {}", err),
    }
}

pub fn detach(port: i32) {
    tokio::task::spawn_blocking(move || {
        if let Err(err) = Command::new("usbip")
            .args(["detach", "-p", &port.to_string()])
            .output()
        {
            log::error!("usb detach: failed to run usbip: {}", err);
        }
    });
}

async fn run_channel(session: Session<FlutterHandler>, bus_id: String, socket: TcpStream) {
    let id = FlutterHandler::next_usb_channel_id();
    let (tx, mut rx) = mpsc::unbounded_channel::<Inbound>();
    session.ui_handler.register_usb_forward_channel(id, tx);
    session.usb_open_forward(id, bus_id);

    let success = loop {
        match rx.recv().await {
            Some(Inbound::Opened { success, message }) => {
                if !success {
                    log::error!("usb attach: remote refused channel {}: {}", id, message);
                }
                break success;
            }
            Some(_) => continue,
            None => {
                session.ui_handler.unregister_usb_forward_channel(id);
                return;
            }
        }
    };
    if !success {
        session.ui_handler.unregister_usb_forward_channel(id);
        return;
    }

    let (mut reader, mut writer) = socket.into_split();
    let session_read = session.clone();
    let to_tunnel = tokio::spawn(async move {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) | Err(_) => {
                    session_read.usb_close_forward(id);
                    return;
                }
                Ok(n) => session_read.usb_forward_data(id, Bytes::copy_from_slice(&buf[..n])),
            }
        }
    });

    while let Some(msg) = rx.recv().await {
        match msg {
            Inbound::Data(data) => {
                if writer.write_all(&data).await.is_err() {
                    break;
                }
            }
            Inbound::Closed | Inbound::Opened { .. } => break,
        }
    }
    to_tunnel.abort();
    session.ui_handler.unregister_usb_forward_channel(id);
}
