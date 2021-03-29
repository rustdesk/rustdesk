use crate::{bail, bytes_codec::BytesCodec, ResultType};
use bytes::{BufMut, Bytes, BytesMut};
use futures::SinkExt;
use protobuf::Message;
use sodiumoxide::crypto::secretbox::{self, Key, Nonce};
use std::{
    io::{Error, ErrorKind},
    ops::{Deref, DerefMut},
};
use tokio::{
    net::{TcpListener, TcpStream, ToSocketAddrs},
    stream::StreamExt,
};
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

impl FramedStream {
    pub async fn new<T: ToSocketAddrs, T2: ToSocketAddrs>(
        remote_addr: T,
        local_addr: T2,
        ms_timeout: u64,
    ) -> ResultType<Self> {
        for local_addr in local_addr.to_socket_addrs().await? {
            for remote_addr in remote_addr.to_socket_addrs().await? {
                if let Ok(stream) = super::timeout(
                    ms_timeout,
                    TcpStream::connect_std(
                        super::new_socket(local_addr, true, true)?.into_tcp_stream(),
                        &remote_addr,
                    ),
                )
                .await?
                {
                    return Ok(Self(Framed::new(stream, BytesCodec::new()), None));
                }
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
        nonce.0[..std::mem::size_of_val(&seqnum)].copy_from_slice(&seqnum.to_ne_bytes());
        nonce
    }
}

const DEFAULT_BACKLOG: i32 = 128;

#[allow(clippy::never_loop)]
pub async fn new_listener<T: ToSocketAddrs>(addr: T, reuse: bool) -> ResultType<TcpListener> {
    if !reuse {
        Ok(TcpListener::bind(addr).await?)
    } else {
        for addr in addr.to_socket_addrs().await? {
            let socket = super::new_socket(addr, true, true)?;
            socket.listen(DEFAULT_BACKLOG)?;
            return Ok(TcpListener::from_std(socket.into_tcp_listener())?);
        }
        bail!("could not resolve to any address");
    }
}
