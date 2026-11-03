//! macOS: IOKit power assertions (`IOPMAssertionCreateWithName`), and
//! CoreGraphics events for the input nudge.
//!
//! `PreventUserIdleSystemSleep` and `PreventUserIdleDisplaySleep` only block
//! *idle* sleep; closing the lid or choosing Sleep still works (SPEC PB-4).
//! `PreventSystemSleep` is never used. Assertions belong to the process, and
//! `powerd` drops them when it exits (SPEC PB-3). Inspect them with
//! `pmset -g assertions`.

use std::ffi::{CString, c_char, c_void};
use std::time::Duration;

use super::{PowerOptions, REASON};
use crate::adapter::{AdapterError, InputUnavailable};

pub(super) type CFStringRef = *const c_void;
type IOPMAssertionID = u32;
type IOReturn = i32;

const K_IOPM_ASSERTION_LEVEL_ON: u32 = 255;
const K_IO_RETURN_SUCCESS: IOReturn = 0;
const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(
        alloc: *const c_void,
        c_str: *const c_char,
        encoding: u32,
    ) -> CFStringRef;
    pub(super) fn CFRelease(cf: *const c_void);
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPMAssertionCreateWithName(
        assertion_type: CFStringRef,
        level: u32,
        name: CFStringRef,
        id: *mut IOPMAssertionID,
    ) -> IOReturn;
    fn IOPMAssertionRelease(id: IOPMAssertionID) -> IOReturn;
}

/// An owned `CFString`, released on drop.
pub(super) struct CfString(CFStringRef);

impl CfString {
    pub(super) fn new(text: &str) -> Result<Self, AdapterError> {
        let c_text = CString::new(text).map_err(|_| AdapterError("Invalid string".into()))?;
        let cf = unsafe {
            CFStringCreateWithCString(std::ptr::null(), c_text.as_ptr(), K_CF_STRING_ENCODING_UTF8)
        };
        if cf.is_null() {
            return Err(AdapterError("CFStringCreateWithCString failed".into()));
        }
        Ok(Self(cf))
    }

    pub(super) fn as_ptr(&self) -> CFStringRef {
        self.0
    }
}

impl Drop for CfString {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) };
    }
}

pub struct PowerAssertion {
    ids: Vec<IOPMAssertionID>,
}

impl PowerAssertion {
    pub fn acquire(options: PowerOptions) -> Result<Self, AdapterError> {
        let mut assertion = Self {
            ids: Vec::with_capacity(2),
        };
        assertion.create("PreventUserIdleSystemSleep")?;
        if options.keep_display_on {
            assertion.create("PreventUserIdleDisplaySleep")?;
        }
        Ok(assertion)
    }

    fn create(&mut self, kind: &str) -> Result<(), AdapterError> {
        let kind_cf = CfString::new(kind)?;
        let name = CfString::new(REASON)?;
        let mut id: IOPMAssertionID = 0;
        let result = unsafe {
            IOPMAssertionCreateWithName(
                kind_cf.as_ptr(),
                K_IOPM_ASSERTION_LEVEL_ON,
                name.as_ptr(),
                &mut id,
            )
        };
        if result != K_IO_RETURN_SUCCESS {
            return Err(AdapterError(format!(
                "IOPMAssertionCreateWithName({kind}) failed: {result:#010x}"
            )));
        }
        self.ids.push(id);
        Ok(())
    }
}

impl Drop for PowerAssertion {
    fn drop(&mut self) {
        for id in self.ids.drain(..) {
            unsafe { IOPMAssertionRelease(id) };
        }
    }
}

// ── Input ────────────────────────────────────────────────────────────────

type CGEventRef = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
struct CGPoint {
    x: f64,
    y: f64,
}

const K_CG_HID_EVENT_TAP: u32 = 0;
const K_CG_EVENT_MOUSE_MOVED: u32 = 5;
const K_CG_EVENT_FLAGS_CHANGED: u32 = 12;
const K_CG_MOUSE_BUTTON_LEFT: u32 = 0;
const K_CG_EVENT_FLAG_MASK_SHIFT: u64 = 0x0002_0000;
const K_CG_EVENT_SOURCE_STATE_HID_SYSTEM: i32 = 1;
const K_CG_ANY_INPUT_EVENT_TYPE: u32 = !0;
const K_VK_SHIFT: u16 = 0x38;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreate(source: *const c_void) -> CGEventRef;
    fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
    fn CGEventCreateMouseEvent(
        source: *const c_void,
        event_type: u32,
        location: CGPoint,
        button: u32,
    ) -> CGEventRef;
    fn CGEventCreateKeyboardEvent(
        source: *const c_void,
        keycode: u16,
        key_down: bool,
    ) -> CGEventRef;
    fn CGEventSetType(event: CGEventRef, event_type: u32);
    fn CGEventSetFlags(event: CGEventRef, flags: u64);
    fn CGEventPost(tap: u32, event: CGEventRef);
    fn CGWarpMouseCursorPosition(point: CGPoint) -> i32;
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        count: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *const c_void;
    static kCFTypeDictionaryKeyCallBacks: c_void;
    static kCFTypeDictionaryValueCallBacks: c_void;
    static kCFBooleanTrue: *const c_void;
}

/// An owned `CGEventRef`, released on drop.
struct Event(CGEventRef);

impl Event {
    fn new(event: CGEventRef, what: &str) -> Result<Self, AdapterError> {
        if event.is_null() {
            return Err(AdapterError(format!("{what} failed")));
        }
        Ok(Self(event))
    }

    fn post(&self) {
        unsafe { CGEventPost(K_CG_HID_EVENT_TAP, self.0) };
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) };
    }
}

fn pointer_location() -> Result<CGPoint, AdapterError> {
    let event = Event::new(unsafe { CGEventCreate(std::ptr::null()) }, "CGEventCreate")?;
    Ok(unsafe { CGEventGetLocation(event.0) })
}

fn mouse_moved(to: CGPoint) -> Result<Event, AdapterError> {
    Event::new(
        unsafe {
            CGEventCreateMouseEvent(
                std::ptr::null(),
                K_CG_EVENT_MOUSE_MOVED,
                to,
                K_CG_MOUSE_BUTTON_LEFT,
            )
        },
        "CGEventCreateMouseEvent",
    )
}

fn shift(down: bool) -> Result<Event, AdapterError> {
    let event = Event::new(
        unsafe { CGEventCreateKeyboardEvent(std::ptr::null(), K_VK_SHIFT, down) },
        "CGEventCreateKeyboardEvent",
    )?;
    // A modifier on its own is a flags change, not a key press.
    unsafe {
        CGEventSetType(event.0, K_CG_EVENT_FLAGS_CHANGED);
        CGEventSetFlags(event.0, if down { K_CG_EVENT_FLAG_MASK_SHIFT } else { 0 });
    }
    Ok(event)
}

/// Input through CoreGraphics events, which need the Accessibility
/// permission (SPEC §6). Idle time comes from the HID event source.
pub struct Input;

impl Input {
    pub fn new() -> Self {
        Self
    }

    pub fn availability(&self) -> Result<(), InputUnavailable> {
        if unsafe { AXIsProcessTrusted() } != 0 {
            Ok(())
        } else {
            Err(InputUnavailable::NeedsPermission)
        }
    }

    /// Moves the pointer 1 px right and back to the same absolute position.
    pub fn nudge_pointer(&mut self) -> Result<(), AdapterError> {
        let start = pointer_location()?;
        mouse_moved(CGPoint {
            x: start.x + 1.0,
            y: start.y,
        })?
        .post();
        mouse_moved(start)?.post();
        if pointer_location()? != start {
            unsafe { CGWarpMouseCursorPosition(start) };
        }
        Ok(())
    }

    /// A Shift tap: no character and no default action. F15 is avoided here
    /// because macOS maps it to display brightness on some keyboards.
    pub fn nudge_key(&mut self) -> Result<(), AdapterError> {
        let down = shift(true)?;
        let up = shift(false)?;
        down.post();
        up.post();
        Ok(())
    }

    pub fn idle_time(&mut self) -> Option<Duration> {
        let seconds = unsafe {
            CGEventSourceSecondsSinceLastEventType(
                K_CG_EVENT_SOURCE_STATE_HID_SYSTEM,
                K_CG_ANY_INPUT_EVENT_TYPE,
            )
        };
        (seconds.is_finite() && seconds >= 0.0).then(|| Duration::from_secs_f64(seconds))
    }
}

/// Shows the system prompt that lists the app under Accessibility, so the
/// user only has to switch it on. Returns whether it is already granted.
pub fn request_accessibility() -> bool {
    unsafe {
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let options = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &raw const kCFTypeDictionaryKeyCallBacks,
            &raw const kCFTypeDictionaryValueCallBacks,
        );
        let trusted = AXIsProcessTrustedWithOptions(options) != 0;
        if !options.is_null() {
            CFRelease(options);
        }
        trusted
    }
}

pub fn open_accessibility_settings() -> Result<(), AdapterError> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn()
        .map(drop)
        .map_err(|error| AdapterError(format!("Could not open System Settings: {error}")))
}
