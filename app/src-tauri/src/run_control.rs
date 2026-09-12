//! In-flight chat turn cancellation. One active run at a time.
//! A second begin is rejected; it never replaces a live guard.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::Notify;

pub struct RunGuard {
    pub conversation_id: String,
    cancelled: AtomicBool,
    notify: Notify,
}

impl std::fmt::Debug for RunGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunGuard")
            .field("conversation_id", &self.conversation_id)
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

impl RunGuard {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub async fn cancelled(&self) {
        loop {
            let notified = self.notify.notified();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusyRun {
    pub conversation_id: String,
}

#[derive(Default)]
pub struct RunControl {
    active: Mutex<Option<Arc<RunGuard>>>,
}

impl RunControl {
    /// Start a turn, or reject if another uncancelled turn is live.
    pub fn try_begin(&self, conversation_id: &str) -> Result<Arc<RunGuard>, BusyRun> {
        let mut lock = self.active.lock().expect("run control");
        if let Some(existing) = lock.as_ref() {
            if !existing.is_cancelled() {
                return Err(BusyRun {
                    conversation_id: existing.conversation_id.clone(),
                });
            }
        }
        let guard = Arc::new(RunGuard {
            conversation_id: conversation_id.to_string(),
            cancelled: AtomicBool::new(false),
            notify: Notify::new(),
        });
        *lock = Some(guard.clone());
        Ok(guard)
    }

    pub fn active(&self) -> Option<Arc<RunGuard>> {
        self.active.lock().expect("run control").clone()
    }

    pub fn cancel(&self, conversation_id: Option<&str>) -> bool {
        let lock = self.active.lock().expect("run control");
        match lock.as_ref() {
            Some(g)
                if conversation_id
                    .map(|id| id == g.conversation_id)
                    .unwrap_or(true) =>
            {
                g.cancel();
                true
            }
            _ => false,
        }
    }

    pub fn end(&self, guard: &Arc<RunGuard>) {
        let mut lock = self.active.lock().expect("run control");
        if lock.as_ref().is_some_and(|g| Arc::ptr_eq(g, guard)) {
            *lock = None;
        }
    }
}

pub struct RunScope<'a> {
    control: &'a RunControl,
    pub guard: Arc<RunGuard>,
}

impl<'a> RunScope<'a> {
    pub fn try_start(control: &'a RunControl, conversation_id: &str) -> Result<Self, BusyRun> {
        Ok(Self {
            control,
            guard: control.try_begin(conversation_id)?,
        })
    }
}

impl Drop for RunScope<'_> {
    fn drop(&mut self) {
        self.control.end(&self.guard);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_turn_is_rejected_while_first_is_live() {
        let control = RunControl::default();
        let first = control.try_begin("c1").expect("first turn");
        let busy = match control.try_begin("c2") {
            Err(busy) => busy,
            Ok(_) => panic!("second turn"),
        };
        assert_eq!(busy.conversation_id, "c1");
        assert!(!first.is_cancelled());
        assert!(control.active().is_some_and(|g| Arc::ptr_eq(&g, &first)));
    }

    #[test]
    fn replacing_the_slot_never_happens_so_no_orphan_guard() {
        let control = RunControl::default();
        let first = control.try_begin("c1").unwrap();
        assert!(control.try_begin("c1").is_err());
        control.end(&first);
        let second = control.try_begin("c2").expect("after end");
        assert_eq!(second.conversation_id, "c2");
        assert!(control.active().is_some_and(|g| Arc::ptr_eq(&g, &second)));
    }

    #[test]
    fn cancel_then_begin_is_allowed() {
        let control = RunControl::default();
        let first = control.try_begin("c1").unwrap();
        first.cancel();
        let second = control.try_begin("c2").expect("cancelled slot");
        assert_eq!(second.conversation_id, "c2");
        assert!(first.is_cancelled());
        assert!(!second.is_cancelled());
    }
}
