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

thread_local! {
    static CONTEXT: RefCell<Option<(u128, usize, u64)>> = const { RefCell::new(None) };
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

pub struct ContextGuard(Option<(u128, usize, u64)>);

impl Drop for ContextGuard {
    fn drop(&mut self) {
        CONTEXT.with(|context| *context.borrow_mut() = self.0.take());
    }
}

pub fn enter(session: u128, display: usize) -> ContextGuard {
    let target = targets()
        .as_ref()
        .and_then(|targets| targets.get(&(session, display)).copied());
    ContextGuard(CONTEXT.with(|context| {
        context
            .borrow_mut()
            .replace((session, display, target.map_or(0, |target| target.generation)))
    }))
}

pub fn current_surface() -> Option<(u64, u64)> {
    current_target().map(|target| (target.surface_id, target.generation))
}

pub fn current_size() -> (i32, i32) {
    current_target().map_or((128, 128), |target| (target.width, target.height))
}

fn current_target() -> Option<RenderTarget> {
    CONTEXT.with(|context| {
        let (session, display, generation) = context.borrow().as_ref().copied()?;
        let target = targets()
            .as_ref()
            .and_then(|targets| targets.get(&(session, display)).copied())?;
        (target.generation == generation).then_some(target)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaced_or_unregistered_surface_is_not_reused() {
        let session = 42;
        let first = register(session, 0, 100, 1920, 1080);
        let _context = enter(session, 0);
        assert_eq!(current_surface(), Some((100, first)));
        assert_eq!(current_size(), (1920, 1080));
        let second = register(session, 0, 200, 1280, 720);
        assert_ne!(first, second);
        assert_eq!(current_surface(), None);
        assert_eq!(enter(session, 0).0, Some((session, 0, second)));
        unregister(session);
        assert_eq!(current_surface(), None);
    }
}
