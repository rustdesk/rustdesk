use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, MutexGuard,
    },
};

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy)]
struct RenderTarget {
    surface_id: u64,
    generation: u64,
    width: i32,
    height: i32,
}

static TARGETS: Mutex<Option<HashMap<(u128, usize), RenderTarget>>> = Mutex::new(None);

// The context only pins (session, display); the generation stays live so a
// re-registered surface (new generation) is picked up by the running thread.
thread_local! {
    static CONTEXT: RefCell<Option<(u128, usize)>> = const { RefCell::new(None) };
}

fn targets() -> MutexGuard<'static, Option<HashMap<(u128, usize), RenderTarget>>> {
    TARGETS.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn register(
    session: u128,
    display: usize,
    surface_id: u64,
    width: i32,
    height: i32,
) -> u64 {
    let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
    targets().get_or_insert_with(HashMap::new).insert(
        (session, display),
        RenderTarget {
            surface_id,
            generation,
            width,
            height,
        },
    );
    generation
}

pub fn unregister(session: u128) {
    if let Some(targets) = targets().as_mut() {
        targets.retain(|(id, _), _| *id != session);
    }
}

pub struct ContextGuard(Option<(u128, usize)>);

impl Drop for ContextGuard {
    fn drop(&mut self) {
        CONTEXT.with(|context| *context.borrow_mut() = self.0.take());
    }
}

pub fn enter(session: u128, display: usize) -> ContextGuard {
    ContextGuard(CONTEXT.with(|context| context.borrow_mut().replace((session, display))))
}

pub fn current_surface() -> Option<(u64, u64)> {
    current_target().map(|target| (target.surface_id, target.generation))
}

pub fn current_size() -> (i32, i32) {
    current_target().map_or((128, 128), |target| (target.width, target.height))
}

fn current_target() -> Option<RenderTarget> {
    CONTEXT.with(|context| {
        let (session, display) = context.borrow().as_ref().copied()?;
        targets()
            .as_ref()
            .and_then(|targets| targets.get(&(session, display)).copied())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_tracks_latest_target() {
        let session = 42;
        let first = register(session, 0, 100, 1920, 1080);
        let _context = enter(session, 0);
        assert_eq!(current_surface(), Some((100, first)));
        assert_eq!(current_size(), (1920, 1080));
        let second = register(session, 0, 200, 1280, 720);
        assert_ne!(first, second);
        // A running video thread picks up the replacement generation; stale
        // decoders are rejected by target_is_current() on their own generation.
        assert_eq!(current_surface(), Some((200, second)));
        assert_eq!(current_size(), (1280, 720));
        assert_eq!(enter(session, 0).0, Some((session, 0)));
        unregister(session);
        assert_eq!(current_surface(), None);
    }
}
