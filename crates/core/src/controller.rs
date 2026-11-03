use std::time::{Duration, Instant};

use crate::adapter::{AdapterError, Capabilities, PlatformAdapter};

/// Who turned simulation on. Timers and schedules join this in Phase 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Manual,
}

#[derive(Clone, Copy, Debug)]
enum State {
    Off,
    On { source: Source, since: Instant },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    pub source: Option<Source>,
    pub elapsed: Duration,
    pub capabilities: Capabilities,
}

impl Status {
    pub fn is_on(&self) -> bool {
        self.source.is_some()
    }
}

/// Owns the `OFF -> ON(source) -> OFF` state machine and its adapter.
pub struct Controller<A: PlatformAdapter> {
    adapter: A,
    state: State,
}

impl<A: PlatformAdapter> Controller<A> {
    /// Starts OFF, whatever the adapter can do.
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            state: State::Off,
        }
    }

    pub fn turn_on(&mut self, source: Source) -> Result<(), AdapterError> {
        if matches!(self.state, State::Off) {
            self.adapter.acquire()?;
            self.state = State::On {
                source,
                since: Instant::now(),
            };
        }
        Ok(())
    }

    pub fn turn_off(&mut self) -> Result<(), AdapterError> {
        if matches!(self.state, State::On { .. }) {
            // OFF wins even if the release fails: the OS drops the assertion
            // when the process exits, and the UI must not claim to be ON.
            self.state = State::Off;
            self.adapter.release()?;
        }
        Ok(())
    }

    pub fn status(&self) -> Status {
        let (source, elapsed) = match self.state {
            State::Off => (None, Duration::ZERO),
            State::On { source, since } => (Some(source), since.elapsed()),
        };
        Status {
            source,
            elapsed,
            capabilities: self.adapter.capabilities(),
        }
    }
}

impl<A: PlatformAdapter> Drop for Controller<A> {
    fn drop(&mut self) {
        let _ = self.turn_off();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    #[derive(Default)]
    struct Calls {
        acquire: u32,
        release: u32,
    }

    #[derive(Default)]
    struct FakeAdapter {
        calls: Rc<RefCell<Calls>>,
        fail_acquire: bool,
        fail_release: bool,
    }

    impl PlatformAdapter for FakeAdapter {
        fn acquire(&mut self) -> Result<(), AdapterError> {
            self.calls.borrow_mut().acquire += 1;
            if self.fail_acquire {
                return Err(AdapterError("acquire failed".into()));
            }
            Ok(())
        }

        fn release(&mut self) -> Result<(), AdapterError> {
            self.calls.borrow_mut().release += 1;
            if self.fail_release {
                return Err(AdapterError("release failed".into()));
            }
            Ok(())
        }

        fn nudge(&mut self) -> Result<(), AdapterError> {
            Ok(())
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities {
                power_assertion: true,
                input_nudge: false,
            }
        }
    }

    #[test]
    fn starts_off_without_touching_the_os() {
        let adapter = FakeAdapter::default();
        let calls = adapter.calls.clone();
        let controller = Controller::new(adapter);

        assert!(!controller.status().is_on());
        assert_eq!(calls.borrow().acquire, 0);
    }

    #[test]
    fn repeated_toggles_acquire_and_release_once() {
        let adapter = FakeAdapter::default();
        let calls = adapter.calls.clone();
        let mut controller = Controller::new(adapter);

        controller.turn_on(Source::Manual).unwrap();
        controller.turn_on(Source::Manual).unwrap();
        assert_eq!(controller.status().source, Some(Source::Manual));
        assert_eq!(calls.borrow().acquire, 1);

        controller.turn_off().unwrap();
        controller.turn_off().unwrap();
        assert!(!controller.status().is_on());
        assert_eq!(controller.status().elapsed, Duration::ZERO);
        assert_eq!(calls.borrow().release, 1);
    }

    #[test]
    fn failed_acquire_stays_off() {
        let adapter = FakeAdapter {
            fail_acquire: true,
            ..Default::default()
        };
        let mut controller = Controller::new(adapter);

        assert!(controller.turn_on(Source::Manual).is_err());
        assert!(!controller.status().is_on());
    }

    #[test]
    fn failed_release_still_reports_off() {
        let adapter = FakeAdapter {
            fail_release: true,
            ..Default::default()
        };
        let mut controller = Controller::new(adapter);
        controller.turn_on(Source::Manual).unwrap();

        assert!(controller.turn_off().is_err());
        assert!(!controller.status().is_on());
    }

    #[test]
    fn dropping_while_on_releases() {
        let adapter = FakeAdapter::default();
        let calls = adapter.calls.clone();
        let mut controller = Controller::new(adapter);
        controller.turn_on(Source::Manual).unwrap();

        drop(controller);

        assert_eq!(calls.borrow().release, 1);
    }
}
