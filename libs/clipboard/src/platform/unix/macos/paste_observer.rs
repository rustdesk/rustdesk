use super::pasteboard_context::PasteObserverInfo;
use fsevent::{self, StreamFlags};
use hbb_common::{bail, log, ResultType};
use std::{
    sync::{
        mpsc::{channel, Receiver, RecvTimeoutError, Sender},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

enum FseventControl {
    Start,
    Stop,
    Exit,
}

struct FseventThreadInfo {
    tx: Sender<FseventControl>,
    handle: thread::JoinHandle<()>,
}

pub struct PasteObserver {
    exit: Arc<Mutex<bool>>,
    observer_info: Arc<Mutex<Option<PasteObserverInfo>>>,
    tx_handle_fsevent_thread: Option<FseventThreadInfo>,
    handle_observer_thread: Option<thread::JoinHandle<()>>,
}

impl Drop for PasteObserver {
    fn drop(&mut self) {
        *self.exit.lock().unwrap() = true;
        if let Some(handle_observer_thread) = self.handle_observer_thread.take() {
            handle_observer_thread.join().ok();
        }
        if let Some(tx_handle_fsevent_thread) = self.tx_handle_fsevent_thread.take() {
            tx_handle_fsevent_thread.tx.send(FseventControl::Exit).ok();
            tx_handle_fsevent_thread.handle.join().ok();
        }
    }
}

impl PasteObserver {
    const OBSERVE_TIMEOUT: Duration = Duration::from_secs(30);

    pub fn new() -> Self {
        Self {
            exit: Arc::new(Mutex::new(false)),
            observer_info: Default::default(),
            tx_handle_fsevent_thread: None,
            handle_observer_thread: None,
        }
    }

    pub fn init(&mut self, cb_pasted: fn(&PasteObserverInfo) -> ()) -> ResultType<()> {
        let Some(home_dir) = dirs::home_dir() else {
            bail!("No home dir is set, do not observe.");
        };

        let (tx_observer, rx_observer) = channel::<fsevent::Event>();
        let handle_observer = Self::init_thread_observer(
            self.exit.clone(),
            self.observer_info.clone(),
            rx_observer,
            cb_pasted,
        );
        self.handle_observer_thread = Some(handle_observer);
        let (tx_control, rx_control) = channel::<FseventControl>();
        let handle_fsevent = Self::init_thread_fsevent(
            home_dir.to_string_lossy().to_string(),
            tx_observer,
            rx_control,
        );
        self.tx_handle_fsevent_thread = Some(FseventThreadInfo {
            tx: tx_control,
            handle: handle_fsevent,
        });
        Ok(())
    }

    #[inline]
    fn get_file_from_path(path: &String) -> String {
        let last_slash = path.rfind('/').or_else(|| path.rfind('\\'));
        match last_slash {
            Some(index) => path[index + 1..].to_string(),
            None => path.clone(),
        }
    }

    fn init_thread_observer(
        exit: Arc<Mutex<bool>>,
        observer_info: Arc<Mutex<Option<PasteObserverInfo>>>,
        rx_observer: Receiver<fsevent::Event>,
        cb_pasted: fn(&PasteObserverInfo) -> (),
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || loop {
            match rx_observer.recv_timeout(Duration::from_millis(300)) {
                Ok(event) => {
                    if (event.flag & StreamFlags::ITEM_CREATED) != StreamFlags::NONE
                        && (event.flag & StreamFlags::ITEM_REMOVED) == StreamFlags::NONE
                        && (event.flag & StreamFlags::IS_FILE) != StreamFlags::NONE
                    {
                        let source_file = observer_info
                            .lock()
                            .unwrap()
                            .as_ref()
                            .map(|x| Self::get_file_from_path(&x.source_path));
                        if let Some(source_file) = source_file {
                            let file = Self::get_file_from_path(&event.path);
                            if source_file == file {
                                if let Some(observer_info) = observer_info.lock().unwrap().as_mut()
                                {
                                    observer_info.target_path = event.path.clone();
                                    cb_pasted(observer_info);
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    if *(exit.lock().unwrap()) {
                        break;
                    }
                }
            }
        })
    }

    fn new_fsevent(home_dir: String, tx_observer: Sender<fsevent::Event>) -> fsevent::FsEvent {
        let mut evt = fsevent::FsEvent::new(vec![home_dir.to_string()]);
        evt.observe_async(tx_observer).ok();
        evt
    }

    fn init_thread_fsevent(
        home_dir: String,
        tx_observer: Sender<fsevent::Event>,
        rx_control: Receiver<FseventControl>,
    ) -> thread::JoinHandle<()> {
        log::debug!("fsevent observe dir: {}", &home_dir);
        thread::spawn(move || {
            let mut fsevent = None;
            loop {
                match rx_control.recv_timeout(Self::OBSERVE_TIMEOUT) {
                    Ok(FseventControl::Start) => {
                        if fsevent.is_none() {
                            fsevent =
                                Some(Self::new_fsevent(home_dir.clone(), tx_observer.clone()));
                        }
                    }
                    Ok(FseventControl::Stop) | Err(RecvTimeoutError::Timeout) => {
                        let _ = fsevent.as_mut().map(|e| e.shutdown_observe());
                        fsevent = None;
                    }
                    Ok(FseventControl::Exit) | Err(RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
            log::info!("fsevent thread exit");
            let _ = fsevent.as_mut().map(|e| e.shutdown_observe());
        })
    }

    pub fn start(&mut self, observer_info: PasteObserverInfo) {
        if let Some(tx_handle_fsevent_thread) = self.tx_handle_fsevent_thread.as_ref() {
            self.observer_info.lock().unwrap().replace(observer_info);
            tx_handle_fsevent_thread.tx.send(FseventControl::Start).ok();
        }
    }

    pub fn stop(&mut self) {
        if let Some(tx_handle_fsevent_thread) = &self.tx_handle_fsevent_thread {
            self.observer_info = Default::default();
            tx_handle_fsevent_thread.tx.send(FseventControl::Stop).ok();
        }
    }
}
