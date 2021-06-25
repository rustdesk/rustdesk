use libc::chmod;
use std::ffi::CString;
use std::io::{self, Error};
use tokio::io::*;
use tokio::net::{UnixListener, UnixStream};
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Socket permissions and ownership on UNIX
pub struct SecurityAttributes {
    // read/write permissions for owner, group and others in unix octal.
    mode: Option<u16>
}

impl SecurityAttributes {
    /// New default security attributes.
    pub fn empty() -> Self {
        SecurityAttributes {
            mode: None
        }
    }

    /// New security attributes that allow everyone to connect.
    pub fn allow_everyone_connect(mut self) -> io::Result<Self> {
        self.mode = Some(0o777);
        Ok(self)
    }

    /// Set a custom permission on the socket
    pub fn set_mode(mut self, mode: u16) -> io::Result<Self> {
        self.mode = Some(mode);
        Ok(self)
    }

    /// New security attributes that allow everyone to create.
    pub fn allow_everyone_create() -> io::Result<Self> {
        Ok(SecurityAttributes {
            mode: None
        })
    }

    /// called in unix, after server socket has been created
    /// will apply security attributes to the socket.
     pub(crate) unsafe fn apply_permissions(&self, path: &str) -> io::Result<()> {
        let path = CString::new(path.to_owned())?;
         if let Some(mode) = self.mode {
            if chmod(path.as_ptr(), mode as _) == -1 {
                return Err(Error::last_os_error())
            }
        }

        Ok(())
    }
}

/// Endpoint implementation for unix systems
pub struct Endpoint {
    path: String,
    security_attributes: SecurityAttributes,
}

pub struct Incoming {
    socket: UnixListener,
}

impl Incoming {
    pub async fn next(&mut self) -> Option<io::Result<Connection>> {
        match self.socket.accept().await {
            Ok((stream, _)) => Some(Ok(Connection::wrap(stream))),
            Err(err) => Some(Err(err))
        }
    }
}

impl Endpoint {
    /// Stream of incoming connections
    pub fn incoming(&mut self) -> io::Result<Incoming> {
        unsafe {
            // the call to bind in `inner()` creates the file
            // `apply_permission()` will set the file permissions.
            self.security_attributes.apply_permissions(&self.path)?;
        };
        let socket = self.inner()?;
        Ok(Incoming { socket })
    }

    /// Inner platform-dependant state of the endpoint
    fn inner(&self) -> io::Result<UnixListener> {
        UnixListener::bind(&self.path)
    }

    /// Set security attributes for the connection
    pub fn set_security_attributes(&mut self, security_attributes: SecurityAttributes) {
        self.security_attributes = security_attributes;
    }

    /// Make new connection using the provided path and running event pool
    pub async fn connect<P: AsRef<Path>>(path: P) -> io::Result<Connection> {
        Ok(Connection::wrap(UnixStream::connect(path.as_ref()).await?))
    }

    /// Returns the path of the endpoint.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// New IPC endpoint at the given path
    pub fn new(path: String) -> Self {
        Endpoint {
            path,
            security_attributes: SecurityAttributes::empty(),
        }
    }
}

/// IPC connection.
pub struct Connection {
    inner: UnixStream,
}

impl Connection {
    fn wrap(stream: UnixStream) -> Self {
        Self { inner: stream }
    }
}

impl AsyncRead for Connection {
    fn poll_read(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = Pin::into_inner(self);
        Pin::new(&mut this.inner).poll_read(ctx, buf)
    }
}

impl AsyncWrite for Connection {
    fn poll_write(
        self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize>> {
        let this = Pin::into_inner(self);
        Pin::new(&mut this.inner).poll_write(ctx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Result<()>> {
        let this = Pin::into_inner(self);
        Pin::new(&mut this.inner).poll_flush(ctx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Result<()>> {
        let this = Pin::into_inner(self);
        Pin::new(&mut this.inner).poll_shutdown(ctx)
    }
}

pub type ConnectionClient = Connection;
