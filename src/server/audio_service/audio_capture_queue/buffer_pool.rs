use hbb_common::anyhow::{bail, Result};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};

const INDEX_BITS: u32 = 4;
pub(super) const MAX_BUFFERS: usize = (u64::BITS / (INDEX_BITS + 1)) as usize;
const READY_BITS: u32 = INDEX_BITS * MAX_BUFFERS as u32;
const READY_MASK: u64 = (1 << READY_BITS) - 1;
const INDEX_MASK: u64 = (1 << INDEX_BITS) - 1;

pub(super) struct BufferPool {
    // Low nibbles hold ready indices plus one, oldest first. High bits mark free slots.
    // Claiming an index transfers exclusive access to its slot. Release the slot's
    // mutex before publishing that index again, so these data locks never contend.
    state: AtomicU64,
    pub(super) slots: Vec<Mutex<(usize, Vec<f32>)>>,
}

impl BufferPool {
    pub(super) fn new(capacity: usize, max_samples: usize) -> Result<Self> {
        if capacity == 0 || capacity > MAX_BUFFERS || max_samples == 0 {
            bail!("Audio capture requires 1..={MAX_BUFFERS} buffers and a nonzero packet size");
        }
        let slots: Vec<_> = (0..capacity)
            .map(|_| Mutex::new((0, Vec::with_capacity(max_samples))))
            .collect();
        for slot in &slots {
            // Initialize mutexes here, including on platforms with lazy allocation.
            drop(slot.lock().unwrap());
        }
        Ok(Self {
            state: AtomicU64::new(((1 << capacity) - 1) << READY_BITS),
            slots,
        })
    }

    pub(super) fn claim(&self, prefer_available: bool) -> Option<(usize, bool)> {
        let mut state = self.state.load(Ordering::Acquire);
        loop {
            let available = if prefer_available {
                state >> READY_BITS
            } else {
                0
            };
            let (index, next, was_ready) = if available != 0 {
                let index = available.trailing_zeros() as usize;
                (index, state & !(1 << (READY_BITS + index as u32)), false)
            } else {
                let first = state & INDEX_MASK;
                if first == 0 {
                    return None;
                }
                let next = (state & !READY_MASK) | ((state & READY_MASK) >> INDEX_BITS);
                (first as usize - 1, next, true)
            };
            // The sender is the only publisher. While it claims or publishes a slot,
            // the worker can only consume/recycle the bounded set of existing slots.
            // Strong CAS failures therefore cannot make the callback wait for it.
            match self
                .state
                .compare_exchange(state, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return Some((index, was_ready)),
                Err(current) => state = current,
            }
        }
    }

    pub(super) fn write(&self, index: usize, sequence: usize, input: &[f32]) {
        let mut slot = self.slots[index].lock().unwrap();
        slot.0 = sequence;
        slot.1.clear();
        slot.1.extend_from_slice(input);
    }

    pub(super) fn publish(&self, index: usize) -> usize {
        let mut state = self.state.load(Ordering::Acquire);
        loop {
            let bits = u64::BITS - (state & READY_MASK).leading_zeros();
            let queued = bits.div_ceil(INDEX_BITS);
            let next = state | ((index as u64 + 1) << (queued * INDEX_BITS));
            match self
                .state
                .compare_exchange(state, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return queued as usize + 1,
                Err(current) => state = current,
            }
        }
    }

    pub(super) fn take_packet(&self, index: usize) -> (usize, Vec<f32>) {
        let mut slot = self.slots[index].lock().unwrap();
        (slot.0, std::mem::take(&mut slot.1))
    }

    pub(super) fn store_recycled(&self, index: usize, buffer: Vec<f32>) {
        self.slots[index].lock().unwrap().1 = buffer;
    }

    pub(super) fn publish_available(&self, index: usize) {
        self.state
            .fetch_or(1 << (READY_BITS + index as u32), Ordering::Release);
    }

    pub(super) fn is_empty(&self) -> bool {
        self.state.load(Ordering::Acquire) & READY_MASK == 0
    }

    #[cfg(test)]
    pub(super) fn available_len(&self) -> usize {
        (self.state.load(Ordering::Acquire) >> READY_BITS).count_ones() as usize
    }
}
