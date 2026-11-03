//! Windows: a power request object (`PowerCreateRequest` / `PowerSetRequest`).
//!
//! SPEC §6 names `SetThreadExecutionState`, but that flag is scoped to the
//! calling thread, and the app turns ON and OFF from whichever thread handles
//! the event. A power request is a kernel handle instead: any thread can set
//! and clear it, and Windows closes it when the process exits, so a crash
//! still releases it (SPEC PB-3). Inspect it with `powercfg /requests`.
//!
//! `PowerRequestSystemRequired` only blocks *idle* sleep. Closing the lid or
//! choosing Sleep still works (SPEC PB-4). Away mode is never requested.

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Power::{
    POWER_REQUEST_TYPE, PowerClearRequest, PowerCreateRequest, PowerRequestDisplayRequired,
    PowerRequestSystemRequired, PowerSetRequest,
};
use windows_sys::Win32::System::Threading::{
    POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0,
};

use super::{PowerOptions, REASON};
use crate::adapter::AdapterError;

const POWER_REQUEST_CONTEXT_VERSION: u32 = 0;

pub struct PowerAssertion {
    handle: HANDLE,
    requests: Vec<POWER_REQUEST_TYPE>,
}

// The handle is a process-wide kernel object, not tied to a thread.
unsafe impl Send for PowerAssertion {}

impl PowerAssertion {
    pub fn acquire(options: PowerOptions) -> Result<Self, AdapterError> {
        // Windows copies the reason string, so it only has to live for the call.
        let mut reason: Vec<u16> = REASON.encode_utf16().chain(Some(0)).collect();
        let context = REASON_CONTEXT {
            Version: POWER_REQUEST_CONTEXT_VERSION,
            Flags: POWER_REQUEST_CONTEXT_SIMPLE_STRING,
            Reason: REASON_CONTEXT_0 {
                SimpleReasonString: reason.as_mut_ptr(),
            },
        };

        let handle = unsafe { PowerCreateRequest(&context) };
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return Err(last_error("PowerCreateRequest"));
        }

        // From here on, Drop clears whatever was set and closes the handle.
        let mut assertion = Self {
            handle,
            requests: Vec::with_capacity(2),
        };
        assertion.set(PowerRequestSystemRequired)?;
        if options.keep_display_on {
            assertion.set(PowerRequestDisplayRequired)?;
        }
        Ok(assertion)
    }

    fn set(&mut self, request: POWER_REQUEST_TYPE) -> Result<(), AdapterError> {
        if unsafe { PowerSetRequest(self.handle, request) } == 0 {
            return Err(last_error("PowerSetRequest"));
        }
        self.requests.push(request);
        Ok(())
    }
}

impl Drop for PowerAssertion {
    fn drop(&mut self) {
        for request in self.requests.drain(..) {
            unsafe { PowerClearRequest(self.handle, request) };
        }
        unsafe { CloseHandle(self.handle) };
    }
}

fn last_error(call: &str) -> AdapterError {
    AdapterError(format!(
        "{call} failed: {}",
        std::io::Error::last_os_error()
    ))
}
