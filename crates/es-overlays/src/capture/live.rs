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
    applied: Rc<Cell<u64>>,
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
            applied: Rc::default(),
        }
    }

    /// The tag for input sent now: the last switch the pick has accepted.
    pub fn stamp(&self) -> impl Fn() -> u64 + Clone + 'static {
        let applied = self.applied.clone();
        move || applied.get()
    }

    /// Whether `tag` is as new as the latest switch; accepting a switch makes later input current.
    pub fn accept(&self, tag: u64) -> bool {
        let current = tag == self.generation.get();
        if current {
            self.applied.set(tag);
        }
        current
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
        let (tx, rx) = async_channel::unbounded();
        let listening = Listening::start(&slot, tx);
        let before = listening.stamp()();
        switch(&slot, "window");
        switch(&slot, "text");
        let first = rx.try_recv().map(|(tag, _)| tag);
        assert_eq!(first.map(|tag| listening.accept(tag)), Ok(false));
        assert!(!listening.accept(before));
    }

    // A switch to the view already shown rebuilds nothing, so the input after it must be current
    // without a new subscription, or Esc and every drag are dropped.
    #[test]
    fn input_after_a_switch_that_changes_no_view_is_current() {
        let slot: Slot<&str> = Rc::default();
        let (tx, rx) = async_channel::unbounded();
        let listening = Listening::start(&slot, tx);
        let stamp = listening.stamp();
        for _ in 0..2 {
            assert!(switch(&slot, "region"));
            let queued = stamp();
            let tag = rx.try_recv().map(|(tag, _)| tag);
            assert_eq!(tag.map(|tag| listening.accept(tag)), Ok(true));
            assert!(!listening.accept(queued), "input sent before the switch");
            assert!(listening.accept(stamp()), "Esc or a drag sent after it");
        }
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
