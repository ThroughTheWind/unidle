use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Instant;

use crate::adapter::PlatformAdapter;
use crate::controller::Controller;

/// A background thread that calls [`Controller::tick`] when the next nudge is
/// due. It sleeps between nudges, so it costs nothing while waiting
/// (SPEC NFR-1). Call [`Ticker::wake`] after anything that changes the
/// schedule, such as turning ON or changing settings.
#[derive(Clone)]
pub struct Ticker {
    signal: Arc<(Mutex<bool>, Condvar)>,
}

impl Ticker {
    pub fn spawn<A>(controller: Arc<Mutex<Controller<A>>>) -> Self
    where
        A: PlatformAdapter + Send + 'static,
    {
        let signal = Arc::new((Mutex::new(false), Condvar::new()));
        let thread_signal = signal.clone();
        thread::Builder::new()
            .name("unidle-ticker".into())
            .spawn(move || {
                let (woken, condvar) = &*thread_signal;
                loop {
                    let wait = match controller.lock() {
                        Ok(mut controller) => controller.tick(Instant::now()),
                        // A panic while holding the lock: stop nudging
                        // rather than act on a half-updated controller.
                        Err(_) => return,
                    };
                    let Ok(guard) = woken.lock() else { return };
                    let Ok((mut guard, _)) =
                        condvar.wait_timeout_while(guard, wait, |woken| !*woken)
                    else {
                        return;
                    };
                    *guard = false;
                }
            })
            .expect("failed to start the nudge thread");
        Self { signal }
    }

    pub fn wake(&self) {
        let (woken, condvar) = &*self.signal;
        if let Ok(mut woken) = woken.lock() {
            *woken = true;
            condvar.notify_one();
        }
    }
}
