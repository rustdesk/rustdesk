use crate::{bail, bytes_codec::BytesCodec, ResultType};
use bytes::{BufMut, Bytes, BytesMut};
use futures::{SinkExt, StreamExt};
use protobuf::Message;
use sodiumoxide::crypto::secretbox::{self, Key, Nonce};
use std::{
    io::{Error, ErrorKind},
    ops::{Deref, DerefMut},
};
use tokio::net::{lookup_host, TcpListener, TcpSocket, TcpStream, ToSocketAddrs};
use tokio_util::codec::Framed;

pub struct FramedStream(Framed<TcpStream, BytesCodec>, Option<(Key, u64, u64)>);

impl Deref for FramedStream {
    type Target = Framed<TcpStream, BytesCodec>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for FramedStream {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

fn new_socket(addr: std::net::SocketAddr, reuse: bool) -> Result<TcpSocket, std::io::Error> {
    let socket = match addr {
        std::net::SocketAddr::V4(..) => TcpSocket::new_v4()?,
        std::net::SocketAddr::V6(..) => TcpSocket::new_v6()?,
    };
    if reuse {
        // windows has no reuse_port, but it's reuse_address
        // almost equals to unix's reuse_port + reuse_address,
        // though may introduce nondeterministic bahavior
        #[cfg(unix)]
        socket.set_reuseport(true)?;
        socket.set_reuseaddr(true)?;
    }
    socket.bind(addr)?;
    Ok(socket)
}

impl FramedStream {
    pub async fn new<T: ToSocketAddrs, T2: ToSocketAddrs>(
        remote_addr: T,
        local_addr: T2,
        ms_timeout: u64,
    ) -> ResultType<Self> {
        for local_addr in lookup_host(&local_addr).await? {
            for remote_addr in lookup_host(&remote_addr).await? {
                let stream = super::timeout(
                    ms_timeout,
                    new_socket(local_addr, true)?.connect(remote_addr),
                )
                .await??;
                return Ok(Self(Framed::new(stream, BytesCodec::new()), None));
            }
        }
        bail!("could not resolve to any address");
    }

    pub fn from(stream: TcpStream) -> Self {
        Self(Framed::new(stream, BytesCodec::new()), None)
    }

    pub fn set_raw(&mut self) {
        self.0.codec_mut().set_raw();
        self.1 = None;
    }

    pub fn is_secured(&self) -> bool {
        self.1.is_some()
    }

    #[inline]
    pub async fn send(&mut self, msg: &impl Message) -> ResultType<()> {
        self.send_raw(msg.write_to_bytes()?).await
    }

    #[inline]
    pub async fn send_raw(&mut self, msg: Vec<u8>) -> ResultType<()> {
        let mut msg = msg;
        if let Some(key) = self.1.as_mut() {
            key.1 += 1;
            let nonce = Self::get_nonce(key.1);
            msg = secretbox::seal(&msg, &nonce, &key.0);
        }
        self.0.send(bytes::Bytes::from(msg)).await?;
        Ok(())
    }

    pub async fn send_bytes(&mut self, bytes: Bytes) -> ResultType<()> {
        self.0.send(bytes).await?;
        Ok(())
    }

    #[inline]
    pub async fn next(&mut self) -> Option<Result<BytesMut, Error>> {
        let mut res = self.0.next().await;
        if let Some(key) = self.1.as_mut() {
            if let Some(Ok(bytes)) = res.as_mut() {
                key.2 += 1;
                let nonce = Self::get_nonce(key.2);
                match secretbox::open(&bytes, &nonce, &key.0) {
                    Ok(res) => {
                        bytes.clear();
                        bytes.put_slice(&res);
                    }
                    Err(()) => {
                        return Some(Err(Error::new(ErrorKind::Other, "decryption error")));
                    }
                }
            }
        }
        res
    }

    #[inline]
    pub async fn next_timeout(&mut self, ms: u64) -> Option<Result<BytesMut, Error>> {
        if let Ok(res) = super::timeout(ms, self.next()).await {
            res
        } else {
            None
        }
    }

    pub fn set_key(&mut self, key: Key) {
        self.1 = Some((key, 0, 0));
    }

    fn get_nonce(seqnum: u64) -> Nonce {
        let mut nonce = Nonce([0u8; secretbox::NONCEBYTES]);
        nonce.0[..std::mem::size_of_val(&seqnum)].copy_from_slice(&seqnum.to_le_bytes());
        nonce
    }
}

const DEFAULT_BACKLOG: u32 = 128;

#[allow(clippy::never_loop)]
pub async fn new_listener<T: ToSocketAddrs>(addr: T, reuse: bool) -> ResultType<TcpListener> {
    if !reuse {
        Ok(TcpListener::bind(addr).await?)
    } else {
        for addr in lookup_host(&addr).await? {
            let socket = new_socket(addr, true)?;
            return Ok(socket.listen(DEFAULT_BACKLOG)?);
        }
        bail!("could not resolve to any address");
    }
}
