//! macOS: IOKit power assertions (`IOPMAssertionCreateWithName`).
//!
//! `PreventUserIdleSystemSleep` and `PreventUserIdleDisplaySleep` only block
//! *idle* sleep; closing the lid or choosing Sleep still works (SPEC PB-4).
//! `PreventSystemSleep` is never used. Assertions belong to the process, and
//! `powerd` drops them when it exits (SPEC PB-3). Inspect them with
//! `pmset -g assertions`.

use std::ffi::{CString, c_char, c_void};

use super::{PowerOptions, REASON};
use crate::adapter::AdapterError;

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
