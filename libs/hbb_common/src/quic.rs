use crate::{allow_err, anyhow::anyhow, ResultType};
use protobuf::Message;
use std::{net::SocketAddr, sync::Arc};
use tokio::{self, stream::StreamExt, sync::mpsc};

const QUIC_HBB: &[&[u8]] = &[b"hbb"];
const SERVER_NAME: &str = "hbb";

type Sender = mpsc::UnboundedSender<Value>;
type Receiver = mpsc::UnboundedReceiver<Value>;

pub fn new_server(socket: std::net::UdpSocket) -> ResultType<(Server, SocketAddr)> {
    let mut transport_config = quinn::TransportConfig::default();
    transport_config.stream_window_uni(0);
    let mut server_config = quinn::ServerConfig::default();
    server_config.transport = Arc::new(transport_config);
    let mut server_config = quinn::ServerConfigBuilder::new(server_config);
    server_config.protocols(QUIC_HBB);
    // server_config.enable_keylog();
    // server_config.use_stateless_retry(true);
    let mut endpoint = quinn::Endpoint::builder();
    endpoint.listen(server_config.build());
    let (end, incoming) = endpoint.with_socket(socket)?;
    Ok((Server { incoming }, end.local_addr()?))
}

pub async fn new_client(local_addr: &SocketAddr, peer: &SocketAddr) -> ResultType<Connection> {
    let mut endpoint = quinn::Endpoint::builder();
    let mut client_config = quinn::ClientConfigBuilder::default();
    client_config.protocols(QUIC_HBB);
    //client_config.enable_keylog();
    endpoint.default_client_config(client_config.build());
    let (endpoint, _) = endpoint.bind(local_addr)?;
    let new_conn = endpoint.connect(peer, SERVER_NAME)?.await?;
    Connection::new_for_client(new_conn.connection).await
}

pub struct Server {
    incoming: quinn::Incoming,
}

impl Server {
    #[inline]
    pub async fn next(&mut self) -> ResultType<Option<Connection>> {
        Connection::new_for_server(&mut self.incoming).await
    }
}

pub struct Connection {
    conn: quinn::Connection,
    tx: quinn::SendStream,
    rx: Receiver,
}

type Value = ResultType<Vec<u8>>;

impl Connection {
    async fn new_for_server(incoming: &mut quinn::Incoming) -> ResultType<Option<Self>> {
        if let Some(conn) = incoming.next().await {
            let quinn::NewConnection {
                connection: conn,
                // uni_streams,
                mut bi_streams,
                ..
            } = conn.await?;
            let (tx, rx) = mpsc::unbounded_channel::<Value>();
            tokio::spawn(async move {
                loop {
                    let stream = bi_streams.next().await;
                    if let Some(stream) = stream {
                        let stream = match stream {
                            Err(e) => {
                                tx.send(Err(e.into())).ok();
                                break;
                            }
                            Ok(s) => s,
                        };
                        let cloned = tx.clone();
                        tokio::spawn(async move {
                            allow_err!(handle_request(stream.1, cloned).await);
                        });
                    } else {
                        tx.send(Err(anyhow!("Reset by the peer"))).ok();
                        break;
                    }
                }
                log::info!("Exit connection outer loop");
            });
            let tx = conn.open_uni().await?;
            Ok(Some(Self { conn, tx, rx }))
        } else {
            Ok(None)
        }
    }

    async fn new_for_client(conn: quinn::Connection) -> ResultType<Self> {
        let (tx, rx_quic) = conn.open_bi().await?;
        let (tx_mpsc, rx) = mpsc::unbounded_channel::<Value>();
        tokio::spawn(async move {
            allow_err!(handle_request(rx_quic, tx_mpsc).await);
        });
        Ok(Self { conn, tx, rx })
    }

    #[inline]
    pub async fn next(&mut self) -> Option<Value> {
        // None is returned when all Sender halves have dropped,
        // indicating that no further values can be sent on the channel.
        self.rx.recv().await
    }

    #[inline]
    pub fn remote_address(&self) -> SocketAddr {
        self.conn.remote_address()
    }

    #[inline]
    pub async fn send_raw(&mut self, bytes: &[u8]) -> ResultType<()> {
        self.tx.write_all(bytes).await?;
        Ok(())
    }

    #[inline]
    pub async fn send(&mut self, msg: &dyn Message) -> ResultType<()> {
        match msg.write_to_bytes() {
            Ok(bytes) => self.send_raw(&bytes).await?,
            err => allow_err!(err),
        }
        Ok(())
    }
}

async fn handle_request(rx: quinn::RecvStream, tx: Sender) -> ResultType<()> {
    Ok(())
}
