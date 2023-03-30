use super::*;
use std::{
    collections::HashSet,
    thread::{self, JoinHandle},
    time,
};

pub trait Service: Send + Sync {
    fn name(&self) -> &'static str;
    fn on_subscribe(&self, sub: ConnInner);
    fn on_unsubscribe(&self, id: i32);
    fn is_subed(&self, id: i32) -> bool;
    fn join(&self);
}

pub trait Subscriber: Default + Send + Sync + 'static {
    fn id(&self) -> i32;
    fn send(&mut self, msg: Arc<Message>);
}

#[derive(Default)]
pub struct ServiceInner<T: Subscriber + From<ConnInner>> {
    name: &'static str,
    handle: Option<JoinHandle<()>>,
    subscribes: HashMap<i32, T>,
    new_subscribes: HashMap<i32, T>,
    active: bool,
    need_snapshot: bool,
}

pub trait Reset {
    fn reset(&mut self);
}

pub struct ServiceTmpl<T: Subscriber + From<ConnInner>>(Arc<RwLock<ServiceInner<T>>>);
pub struct ServiceSwap<T: Subscriber + From<ConnInner>>(ServiceTmpl<T>);
pub type GenericService = ServiceTmpl<ConnInner>;
pub const HIBERNATE_TIMEOUT: u64 = 30;
pub const MAX_ERROR_TIMEOUT: u64 = 1_000;

impl<T: Subscriber + From<ConnInner>> ServiceInner<T> {
    fn send_new_subscribes(&mut self, msg: Arc<Message>) {
        for s in self.new_subscribes.values_mut() {
            s.send(msg.clone());
        }
    }

    fn swap_new_subscribes(&mut self) {
        for (_, s) in self.new_subscribes.drain() {
            self.subscribes.insert(s.id(), s);
        }
        assert!(self.new_subscribes.is_empty());
    }

    #[inline]
    fn has_subscribes(&self) -> bool {
        self.subscribes.len() > 0 || self.new_subscribes.len() > 0
    }
}

impl<T: Subscriber + From<ConnInner>> Service for ServiceTmpl<T> {
    #[inline]
    fn name(&self) -> &'static str {
        self.0.read().unwrap().name
    }

    fn is_subed(&self, id: i32) -> bool {
        self.0.read().unwrap().subscribes.get(&id).is_some()
    }

    fn on_subscribe(&self, sub: ConnInner) {
        let mut lock = self.0.write().unwrap();
        if lock.subscribes.get(&sub.id()).is_some() {
            return;
        }
        if lock.need_snapshot {
            lock.new_subscribes.insert(sub.id(), sub.into());
        } else {
            lock.subscribes.insert(sub.id(), sub.into());
        }
    }

    fn on_unsubscribe(&self, id: i32) {
        let mut lock = self.0.write().unwrap();
        if let None = lock.subscribes.remove(&id) {
            lock.new_subscribes.remove(&id);
        }
    }

    fn join(&self) {
        self.0.write().unwrap().active = false;
        let handle = self.0.write().unwrap().handle.take();
        if let Some(handle) = handle {
            if let Err(e) = handle.join() {
                log::error!("Failed to join thread for service {}, {:?}", self.name(), e);
            }
        }
    }
}

impl<T: Subscriber + From<ConnInner>> Clone for ServiceTmpl<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: Subscriber + From<ConnInner>> ServiceTmpl<T> {
    pub fn new(name: &'static str, need_snapshot: bool) -> Self {
        Self(Arc::new(RwLock::new(ServiceInner::<T> {
            name,
            active: true,
            need_snapshot,
            ..Default::default()
        })))
    }

    #[inline]
    pub fn has_subscribes(&self) -> bool {
        self.0.read().unwrap().has_subscribes()
    }

    #[inline]
    pub fn ok(&self) -> bool {
        let lock = self.0.read().unwrap();
        lock.active && lock.has_subscribes()
    }

    pub fn snapshot<F>(&self, callback: F) -> ResultType<()>
    where
        F: FnMut(ServiceSwap<T>) -> ResultType<()>,
    {
        if self.0.read().unwrap().new_subscribes.len() > 0 {
            log::info!("Call snapshot of {} service", self.name());
            let mut callback = callback;
            callback(ServiceSwap::<T>(self.clone()))?;
        }
        Ok(())
    }

    #[inline]
    pub fn send(&self, msg: Message) {
        self.send_shared(Arc::new(msg));
    }

    pub fn send_to(&self, msg: Message, id: i32) {
        if let Some(s) = self.0.write().unwrap().subscribes.get_mut(&id) {
            s.send(Arc::new(msg));
        }
    }

    pub fn send_to_others(&self, msg: Message, id: i32) {
        let msg = Arc::new(msg);
        let mut lock = self.0.write().unwrap();
        for (sid, s) in lock.subscribes.iter_mut() {
            if *sid != id {
                s.send(msg.clone());
            }
        }
    }

    pub fn send_shared(&self, msg: Arc<Message>) {
        let mut lock = self.0.write().unwrap();
        for s in lock.subscribes.values_mut() {
            s.send(msg.clone());
        }
    }

    pub fn send_video_frame(&self, msg: Message) -> HashSet<i32> {
        self.send_video_frame_shared(Arc::new(msg))
    }

    pub fn send_video_frame_shared(&self, msg: Arc<Message>) -> HashSet<i32> {
        let mut conn_ids = HashSet::new();
        let mut lock = self.0.write().unwrap();
        for s in lock.subscribes.values_mut() {
            s.send(msg.clone());
            conn_ids.insert(s.id());
        }
        conn_ids
    }

    pub fn send_without(&self, msg: Message, sub: i32) {
        let mut lock = self.0.write().unwrap();
        let msg = Arc::new(msg);
        for s in lock.subscribes.values_mut() {
            if sub != s.id() {
                s.send(msg.clone());
            }
        }
    }

    pub fn repeat<S, F>(&self, interval_ms: u64, callback: F)
    where
        F: 'static + FnMut(Self, &mut S) -> ResultType<()> + Send,
        S: 'static + Default + Reset,
    {
        let interval = time::Duration::from_millis(interval_ms);
        let mut callback = callback;
        let sp = self.clone();
        let thread = thread::spawn(move || {
            let mut state = S::default();
            let mut may_reset = false;
            while sp.active() {
                let now = time::Instant::now();
                if sp.has_subscribes() {
                    if let Err(err) = callback(sp.clone(), &mut state) {
                        log::error!("Error of {} service: {}", sp.name(), err);
                        thread::sleep(time::Duration::from_millis(MAX_ERROR_TIMEOUT));
                        #[cfg(windows)]
                        crate::platform::windows::try_change_desktop();
                    }
                    if !may_reset {
                        may_reset = true;
                    }
                } else if may_reset {
                    state.reset();
                    may_reset = false;
                }
                let elapsed = now.elapsed();
                if elapsed < interval {
                    thread::sleep(interval - elapsed);
                }
            }
            log::info!("Service {} exit", sp.name());
        });
        self.0.write().unwrap().handle = Some(thread);
    }

    pub fn run<F>(&self, callback: F)
    where
        F: 'static + FnMut(Self) -> ResultType<()> + Send,
    {
        let sp = self.clone();
        let mut callback = callback;
        let thread = thread::spawn(move || {
            let mut error_timeout = HIBERNATE_TIMEOUT;
            while sp.active() {
                if sp.has_subscribes() {
                    log::debug!("Enter {} service inner loop", sp.name());
                    let tm = time::Instant::now();
                    if let Err(err) = callback(sp.clone()) {
                        log::error!("Error of {} service: {}", sp.name(), err);
                        if tm.elapsed() > time::Duration::from_millis(MAX_ERROR_TIMEOUT) {
                            error_timeout = HIBERNATE_TIMEOUT;
                        } else {
                            error_timeout *= 2;
                        }
                        if error_timeout > MAX_ERROR_TIMEOUT {
                            error_timeout = MAX_ERROR_TIMEOUT;
                        }
                        thread::sleep(time::Duration::from_millis(error_timeout));
                        #[cfg(windows)]
                        crate::platform::windows::try_change_desktop();
                    } else {
                        log::debug!("Exit {} service inner loop", sp.name());
                    }
                }
                thread::sleep(time::Duration::from_millis(HIBERNATE_TIMEOUT));
            }
            log::info!("Service {} exit", sp.name());
        });
        self.0.write().unwrap().handle = Some(thread);
    }

    #[inline]
    pub fn active(&self) -> bool {
        self.0.read().unwrap().active
    }
}

impl<T: Subscriber + From<ConnInner>> ServiceSwap<T> {
    #[inline]
    pub fn send(&self, msg: Message) {
        self.send_shared(Arc::new(msg));
    }

    #[inline]
    pub fn send_shared(&self, msg: Arc<Message>) {
        (self.0).0.write().unwrap().send_new_subscribes(msg);
    }

    #[inline]
    pub fn has_subscribes(&self) -> bool {
        (self.0).0.read().unwrap().subscribes.len() > 0
    }
}

impl<T: Subscriber + From<ConnInner>> Drop for ServiceSwap<T> {
    fn drop(&mut self) {
        (self.0).0.write().unwrap().swap_new_subscribes();
    }
}
