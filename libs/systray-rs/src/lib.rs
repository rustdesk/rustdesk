// Systray Lib
pub mod api;

use std::{
    collections::HashMap,
    error, fmt,
    sync::mpsc::{channel, Receiver},
};

type BoxedError = Box<dyn error::Error + Send + Sync + 'static>;

#[derive(Debug)]
pub enum Error {
    OsError(String),
    NotImplementedError,
    UnknownError,
    Error(BoxedError),
}

impl From<BoxedError> for Error {
    fn from(value: BoxedError) -> Self {
        Error::Error(value)
    }
}

pub struct SystrayEvent {
    menu_index: u32,
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        use self::Error::*;

        match *self {
            OsError(ref err_str) => write!(f, "OsError: {}", err_str),
            NotImplementedError => write!(f, "Functionality is not implemented yet"),
            UnknownError => write!(f, "Unknown error occurrred"),
            Error(ref e) => write!(f, "Error: {}", e),
        }
    }
}

pub struct Application {
    window: api::api::Window,
    menu_idx: u32,
    callback: HashMap<u32, Callback>,
    // Each platform-specific window module will set up its own thread for
    // dealing with the OS main loop. Use this channel for receiving events from
    // that thread.
    rx: Receiver<SystrayEvent>,
    timer: Option<(std::time::Duration, Callback)>,
}

type Callback =
    Box<(dyn FnMut(&mut Application) -> Result<(), BoxedError> + Send + Sync + 'static)>;

fn make_callback<F, E>(mut f: F) -> Callback
where
    F: FnMut(&mut Application) -> Result<(), E> + Send + Sync + 'static,
    E: error::Error + Send + Sync + 'static,
{
    Box::new(move |a: &mut Application| match f(a) {
        Ok(()) => Ok(()),
        Err(e) => Err(Box::new(e) as BoxedError),
    }) as Callback
}

impl Application {
    pub fn new() -> Result<Application, Error> {
        let (event_tx, event_rx) = channel();
        match api::api::Window::new(event_tx) {
            Ok(w) => Ok(Application {
                window: w,
                menu_idx: 0,
                callback: HashMap::new(),
                rx: event_rx,
                timer: None,
            }),
            Err(e) => Err(e),
        }
    }

    pub fn set_timer<F, E>(
        &mut self,
        interval: std::time::Duration,
        callback: F,
    ) -> Result<(), Error>
    where
        F: FnMut(&mut Application) -> Result<(), E> + Send + Sync + 'static,
        E: error::Error + Send + Sync + 'static,
    {
        self.timer = Some((interval, make_callback(callback)));
        Ok(())
    }

    pub fn add_menu_item<F, E>(&mut self, item_name: &str, f: F) -> Result<u32, Error>
    where
        F: FnMut(&mut Application) -> Result<(), E> + Send + Sync + 'static,
        E: error::Error + Send + Sync + 'static,
    {
        let idx = self.menu_idx;
        if let Err(e) = self.window.add_menu_entry(idx, item_name) {
            return Err(e);
        }
        self.callback.insert(idx, make_callback(f));
        self.menu_idx += 1;
        Ok(idx)
    }

    pub fn remove_menu_item(&mut self, pos: u32) {
        self.window.remove_menu_entry(pos);
        self.callback.remove(&pos);
    }

    pub fn add_menu_separator(&mut self) -> Result<u32, Error> {
        let idx = self.menu_idx;
        if let Err(e) = self.window.add_menu_separator(idx) {
            return Err(e);
        }
        self.menu_idx += 1;
        Ok(idx)
    }

    pub fn set_icon_from_file(&self, file: &str) -> Result<(), Error> {
        self.window.set_icon_from_file(file)
    }

    pub fn set_icon_from_resource(&self, resource: &str) -> Result<(), Error> {
        self.window.set_icon_from_resource(resource)
    }

    #[cfg(target_os = "windows")]
    pub fn set_icon_from_buffer(
        &self,
        buffer: &[u8],
        width: u32,
        height: u32,
    ) -> Result<(), Error> {
        self.window.set_icon_from_buffer(buffer, width, height)
    }

    pub fn shutdown(&self) -> Result<(), Error> {
        self.window.shutdown()
    }

    pub fn set_tooltip(&self, tooltip: &str) -> Result<(), Error> {
        self.window.set_tooltip(tooltip)
    }

    pub fn quit(&mut self) {
        self.window.quit()
    }

    pub fn wait_for_message(&mut self) -> Result<(), Error> {
        loop {
            let mut msg = None;
            if let Some((interval, _)) = self.timer.as_ref() {
                match self.rx.recv_timeout(interval.clone()) {
                    Ok(m) => msg = Some(m),
                    Err(_) => {}
                }
            } else {
                match self.rx.recv() {
                    Ok(m) => msg = Some(m),
                    Err(_) => {
                        self.quit();
                        break;
                    }
                }
            }
            if let Some(msg) = msg {
                if let Some(mut f) = self.callback.remove(&msg.menu_index) {
                    f(self)?;
                    self.callback.insert(msg.menu_index, f);
                }
            } else if let Some((interval, mut callback)) = self.timer.take() {
                callback(self)?;
                self.timer = Some((interval, callback));
            }
        }

        Ok(())
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        self.shutdown().ok();
    }
}
