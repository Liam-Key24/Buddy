//! In-flight chat turn cancellation. One active run at a time.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::Notify;

pub struct RunGuard {
    pub conversation_id: String,
    cancelled: AtomicBool,
    notify: Notify,
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

#[derive(Default)]
pub struct RunControl {
    active: Mutex<Option<Arc<RunGuard>>>,
}

impl RunControl {
    pub fn begin(&self, conversation_id: &str) -> Arc<RunGuard> {
        let guard = Arc::new(RunGuard {
            conversation_id: conversation_id.to_string(),
            cancelled: AtomicBool::new(false),
            notify: Notify::new(),
        });
        *self.active.lock().expect("run control") = Some(guard.clone());
        guard
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
    pub fn start(control: &'a RunControl, conversation_id: &str) -> Self {
        Self {
            control,
            guard: control.begin(conversation_id),
        }
    }
}

impl Drop for RunScope<'_> {
    fn drop(&mut self) {
        self.control.end(&self.guard);
    }
}
