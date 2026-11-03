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

use std::time::Duration;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE, POINT};
use windows_sys::Win32::System::Power::{
    POWER_REQUEST_TYPE, PowerClearRequest, PowerCreateRequest, PowerRequestDisplayRequired,
    PowerRequestSystemRequired, PowerSetRequest,
};
use windows_sys::Win32::System::Threading::{
    POWER_REQUEST_CONTEXT_SIMPLE_STRING, REASON_CONTEXT, REASON_CONTEXT_0,
};

use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetLastInputInfo, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_KEYUP, LASTINPUTINFO, MOUSEEVENTF_MOVE, MOUSEINPUT, SendInput, VIRTUAL_KEY, VK_F15,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos};

use super::{PowerOptions, REASON};
use crate::adapter::{AdapterError, InputUnavailable};

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

/// Input through `SendInput`, idle time through `GetLastInputInfo`.
pub struct Input;

impl Input {
    pub fn new() -> Self {
        Self
    }

    pub fn availability(&self) -> Result<(), InputUnavailable> {
        Ok(())
    }

    /// A relative move of 1 px and back. Pointer acceleration can turn a
    /// 1 px move into 0 or 2 px, so the start position is restored exactly
    /// afterwards if the pair did not cancel out.
    pub fn nudge_pointer(&mut self) -> Result<(), AdapterError> {
        let start = cursor_position()?;
        send(&[mouse_move(1), mouse_move(-1)])?;
        let end = cursor_position()?;
        if (end.x, end.y) != (start.x, start.y) {
            unsafe { SetCursorPos(start.x, start.y) };
        }
        Ok(())
    }

    /// F15 down and up. It types nothing and has no default action.
    pub fn nudge_key(&mut self) -> Result<(), AdapterError> {
        send(&[key(VK_F15, 0), key(VK_F15, KEYEVENTF_KEYUP)])
    }

    pub fn idle_time(&mut self) -> Option<Duration> {
        let mut info = LASTINPUTINFO {
            cbSize: size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if unsafe { GetLastInputInfo(&mut info) } == 0 {
            return None;
        }
        // Both are milliseconds since boot and wrap together every 49.7 days.
        let now = unsafe { GetTickCount() };
        Some(Duration::from_millis(u64::from(
            now.wrapping_sub(info.dwTime),
        )))
    }
}

fn cursor_position() -> Result<POINT, AdapterError> {
    let mut point = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut point) } == 0 {
        return Err(last_error("GetCursorPos"));
    }
    Ok(point)
}

fn mouse_move(dx: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy: 0,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_MOVE,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn key(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send(inputs: &[INPUT]) -> Result<(), AdapterError> {
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        )
    };
    // Fewer events than asked means Windows blocked them, for example on the
    // lock screen or over an elevated window.
    if sent as usize != inputs.len() {
        return Err(last_error("SendInput"));
    }
    Ok(())
}
