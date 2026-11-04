use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// The running pick's event line, so a newer request can act on the frames it froze.
pub type Slot<T> = Rc<RefCell<Option<Line<T>>>>;

pub struct Line<T> {
    tx: async_channel::Sender<(u64, T)>,
    generation: Rc<Cell<u64>>,
}

/// Picking lasts while this lives; an event tagged before the latest switch is stale.
pub struct Listening<T> {
    slot: Slot<T>,
    generation: Rc<Cell<u64>>,
}

impl<T> Listening<T> {
    pub fn start(slot: &Slot<T>, tx: async_channel::Sender<(u64, T)>) -> Self {
        let generation = Rc::new(Cell::new(0));
        *slot.borrow_mut() = Some(Line {
            tx,
            generation: generation.clone(),
        });
        Listening {
            slot: slot.clone(),
            generation,
        }
    }

    pub fn tag(&self) -> u64 {
        self.generation.get()
    }

    pub fn is_current(&self, tag: u64) -> bool {
        tag == self.generation.get()
    }
}

impl<T> Drop for Listening<T> {
    fn drop(&mut self) {
        let mut slot = self.slot.borrow_mut();
        if slot
            .as_ref()
            .is_some_and(|line| Rc::ptr_eq(&line.generation, &self.generation))
        {
            *slot = None;
        }
    }
}

/// Sends `event` past everything queued before it; false once nothing is being picked.
pub fn switch<T>(slot: &Slot<T>, event: T) -> bool {
    let slot = slot.borrow();
    let Some(line) = slot.as_ref() else {
        return false;
    };
    let tag = line.generation.get() + 1;
    line.generation.set(tag);
    line.tx.try_send((tag, event)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_switch_reaches_only_a_pick_that_is_still_listening() {
        let slot: Slot<&str> = Rc::default();
        assert!(!switch(&slot, "region"));
        let (tx, rx) = async_channel::unbounded();
        let listening = Listening::start(&slot, tx);
        assert!(switch(&slot, "window"));
        assert_eq!(rx.try_recv().ok(), Some((1, "window")));
        drop(listening);
        assert!(!switch(&slot, "color"));
    }

    #[test]
    fn events_tagged_before_a_switch_are_stale() {
        let slot: Slot<&str> = Rc::default();
        let (tx, _rx) = async_channel::unbounded();
        let listening = Listening::start(&slot, tx);
        let before = listening.tag();
        switch(&slot, "window");
        assert!(!listening.is_current(before));
        assert!(listening.is_current(listening.tag()));
    }

    #[test]
    fn an_ended_pick_never_silences_a_newer_one() {
        let slot: Slot<&str> = Rc::default();
        let old = Listening::start(&slot, async_channel::unbounded().0);
        let (tx, _rx) = async_channel::unbounded();
        let _new = Listening::start(&slot, tx);
        drop(old);
        assert!(switch(&slot, "text"));
    }
}
