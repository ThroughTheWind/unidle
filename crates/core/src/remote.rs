//! The local channel between the `unidle` command line and the running app
//! (SPEC §13). One request line and one response line of JSON per
//! connection.
//!
//! It never leaves the machine and only the same user can reach it: a Unix
//! socket in a folder only that user can open, or a Windows named pipe that
//! refuses remote clients. No network (SPEC NFR-2).

use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, BufReader, Read, Write};

/// What the command line asks the app to do.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase")]
pub enum Request {
    /// Turn ON. Without `nudge`, the power assertion only.
    On {
        nudge: bool,
    },
    Off,
    Status,
    /// ON for `minutes`, or until `until` ("18:00").
    #[serde(rename_all = "camelCase")]
    Timer {
        minutes: Option<u32>,
        until: Option<String>,
        nudge: bool,
    },
}

/// The app's answer: the status after the request, as the window sees it,
/// or why it failed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Response {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<serde_json::Value>,
    /// The same status in words, for a person at a terminal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A request line longer than this is not from the command line.
const MAX_LINE: u64 = 4096;

/// Starts listening on a background thread. Each request is answered by
/// `handle`, on its own thread. Fails when the channel can't be opened; the
/// app still runs, without its command line.
pub fn serve<F>(handle: F) -> io::Result<()>
where
    F: Fn(Request) -> Response + Send + Sync + 'static,
{
    sys::serve(std::sync::Arc::new(handle))
}

/// Sends one request to the running app. `Ok(None)` when no app is
/// listening.
pub fn send(request: &Request) -> io::Result<Option<Response>> {
    let Some(mut stream) = sys::connect()? else {
        return Ok(None);
    };
    let mut line = serde_json::to_string(request).map_err(io::Error::other)?;
    line.push('\n');
    stream.write_all(line.as_bytes())?;
    stream.flush()?;
    let answer = read_line(&mut stream)?;
    serde_json::from_str(&answer)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn read_line(stream: impl Read) -> io::Result<String> {
    let mut line = String::new();
    BufReader::new(stream.take(MAX_LINE)).read_line(&mut line)?;
    if !line.ends_with('\n') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "incomplete message",
        ));
    }
    Ok(line)
}

/// Reads one request from a connection, answers it, and closes it.
fn answer<S: Read + Write>(mut stream: S, handle: &(dyn Fn(Request) -> Response + Send + Sync)) {
    let response = match read_line(&mut stream) {
        Ok(line) => match serde_json::from_str::<Request>(&line) {
            Ok(request) => handle(request),
            Err(error) => Response {
                error: Some(format!("Not a command: {error}")),
                ..Response::default()
            },
        },
        // Nothing to answer: the client went away or sent junk.
        Err(_) => return,
    };
    if let Ok(mut line) = serde_json::to_string(&response) {
        line.push('\n');
        let _ = stream.write_all(line.as_bytes());
        let _ = stream.flush();
    }
}

type Handler = std::sync::Arc<dyn Fn(Request) -> Response + Send + Sync>;

#[cfg(unix)]
mod sys {
    use std::fs;
    use std::io;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::PathBuf;
    use std::time::Duration;

    use super::{Handler, answer};

    /// A client that connects and says nothing is dropped after this. On
    /// Windows each request has its own thread, so a silent client only
    /// holds that one.
    const READ_TIMEOUT: Duration = Duration::from_secs(5);

    /// `$XDG_RUNTIME_DIR` on Linux, else a folder of our own in the temp
    /// folder (per user on macOS already).
    fn socket_path() -> io::Result<PathBuf> {
        if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR").filter(|dir| !dir.is_empty()) {
            return Ok(PathBuf::from(dir).join("unidle.sock"));
        }
        // SAFETY: getuid has no preconditions and cannot fail.
        let uid = unsafe { libc::getuid() };
        let dir = std::env::temp_dir().join(format!("unidle-{uid}"));
        match fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        // Someone else made the folder first, or opened it up: don't use it.
        let metadata = fs::symlink_metadata(&dir)?;
        if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} isn't private to this user", dir.display()),
            ));
        }
        Ok(dir.join("unidle.sock"))
    }

    pub(super) fn serve(handle: Handler) -> io::Result<()> {
        let path = socket_path()?;
        // A socket left by a crash answers nobody; one that answers belongs
        // to a running app.
        if path.exists() {
            if UnixStream::connect(&path).is_ok() {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "another Unidle is listening",
                ));
            }
            fs::remove_file(&path)?;
        }
        let listener = UnixListener::bind(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        std::thread::Builder::new()
            .name("unidle-remote".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let handle = handle.clone();
                    let _ = std::thread::Builder::new()
                        .name("unidle-remote-request".into())
                        .spawn(move || {
                            let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
                            answer(stream, &*handle);
                        });
                }
            })?;
        Ok(())
    }

    pub(super) fn connect() -> io::Result<Option<UnixStream>> {
        let path = socket_path()?;
        match UnixStream::connect(&path) {
            Ok(stream) => {
                stream.set_read_timeout(Some(READ_TIMEOUT * 2))?;
                Ok(Some(stream))
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    pub fn attach_parent_console() {}
}

#[cfg(windows)]
mod sys {
    use std::fs::{File, OpenOptions};
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{FromRawHandle, RawHandle};

    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, GetLastError,
        HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_FIRST_PIPE_INSTANCE, FlushFileBuffers, PIPE_ACCESS_DUPLEX,
    };
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
        PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };

    use super::{Handler, answer};

    /// One pipe per user name. Named pipes are machine-wide, so the name
    /// keeps two signed-in users apart.
    fn pipe_name() -> String {
        let user = std::env::var("USERNAME").unwrap_or_default();
        let user: String = user
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        format!(r"\\.\pipe\unidle-{user}")
    }

    fn create(name: &[u16], first: bool) -> io::Result<HANDLE> {
        let flags = PIPE_ACCESS_DUPLEX
            | if first {
                FILE_FLAG_FIRST_PIPE_INSTANCE
            } else {
                0
            };
        // SAFETY: `name` is NUL-terminated and outlives the call; the
        // default security attributes are used.
        let handle = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                flags,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                4096,
                4096,
                0,
                std::ptr::null(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        Ok(handle)
    }

    /// Closes the pipe instance once the client has read the answer.
    struct Instance(File);

    impl io::Read for Instance {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.0.read(buf)
        }
    }

    impl io::Write for Instance {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            use std::os::windows::io::AsRawHandle;
            // SAFETY: the handle is the open pipe this File owns.
            unsafe { FlushFileBuffers(self.0.as_raw_handle() as HANDLE) };
            Ok(())
        }
    }

    pub(super) fn serve(handle: Handler) -> io::Result<()> {
        let name: Vec<u16> = std::ffi::OsStr::new(&pipe_name())
            .encode_wide()
            .chain(Some(0))
            .collect();
        // Fails if another process already owns the name, so nobody can sit
        // in front of the app.
        let first = create(&name, true)?;
        let first = first as usize;
        std::thread::Builder::new()
            .name("unidle-remote".into())
            .spawn(move || {
                let mut current = first as HANDLE;
                loop {
                    // SAFETY: `current` is a pipe instance we created.
                    let connected = unsafe { ConnectNamedPipe(current, std::ptr::null_mut()) };
                    // SAFETY: no preconditions.
                    if connected == 0 && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED {
                        // SAFETY: we own `current` and drop it here.
                        unsafe { CloseHandle(current) };
                        match create(&name, false) {
                            Ok(next) => {
                                current = next;
                                continue;
                            }
                            Err(_) => return,
                        }
                    }
                    // The next instance exists before this one is handed off,
                    // so the name is never free.
                    let next = match create(&name, false) {
                        Ok(next) => next,
                        Err(_) => return,
                    };
                    // SAFETY: we own `current`; the File closes it.
                    let file = unsafe { File::from_raw_handle(current as RawHandle) };
                    let handle = handle.clone();
                    let _ = std::thread::Builder::new()
                        .name("unidle-remote-request".into())
                        .spawn(move || answer(Instance(file), &*handle));
                    current = next;
                }
            })?;
        Ok(())
    }

    pub(super) fn connect() -> io::Result<Option<File>> {
        let name = pipe_name();
        for _ in 0..50 {
            match OpenOptions::new().read(true).write(true).open(&name) {
                Ok(file) => return Ok(Some(file)),
                Err(error) if error.raw_os_error() == Some(ERROR_FILE_NOT_FOUND as i32) => {
                    return Ok(None);
                }
                Err(error) if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(io::ErrorKind::TimedOut, "Unidle is busy"))
    }

    /// The app is built as a window program, so a terminal that runs
    /// `unidle status` gets no output unless it joins that terminal.
    pub fn attach_parent_console() {
        // SAFETY: no preconditions; fails harmlessly without a parent console.
        unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
    }
}

pub use sys::attach_parent_console;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip_as_one_json_line() {
        for request in [
            Request::On { nudge: false },
            Request::Off,
            Request::Status,
            Request::Timer {
                minutes: Some(30),
                until: None,
                nudge: true,
            },
            Request::Timer {
                minutes: None,
                until: Some("18:00".into()),
                nudge: false,
            },
        ] {
            let line = serde_json::to_string(&request).unwrap();
            assert!(!line.contains('\n'));
            assert_eq!(serde_json::from_str::<Request>(&line).unwrap(), request);
        }
        assert_eq!(
            serde_json::to_string(&Request::Off).unwrap(),
            r#"{"cmd":"off"}"#
        );
    }

    #[test]
    fn answers_one_request_and_refuses_junk() {
        struct Pipe {
            input: io::Cursor<Vec<u8>>,
            output: Vec<u8>,
        }
        impl Read for Pipe {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                self.input.read(buf)
            }
        }
        impl Write for Pipe {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.output.write(buf)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let run = |input: &str| {
            let mut pipe = Pipe {
                input: io::Cursor::new(input.as_bytes().to_vec()),
                output: Vec::new(),
            };
            answer(&mut pipe, &|request| Response {
                status: Some(serde_json::json!({ "asked": format!("{request:?}") })),
                ..Response::default()
            });
            String::from_utf8(pipe.output).unwrap()
        };
        assert_eq!(
            run("{\"cmd\":\"status\"}\n"),
            "{\"status\":{\"asked\":\"Status\"}}\n"
        );
        assert!(run("{\"cmd\":\"reboot\"}\n").contains("\"error\""));
        // No newline: an incomplete request gets no answer.
        assert_eq!(run("{\"cmd\":\"status\"}"), "");
    }

    #[cfg(unix)]
    #[test]
    fn serves_over_a_private_socket() {
        let dir = std::env::temp_dir().join(format!("unidle-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::os::unix::fs::PermissionsExt::from_mode(0o700))
            .unwrap();
        // SAFETY: the only test in this crate that reads XDG_RUNTIME_DIR.
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", &dir) };
        assert_eq!(send(&Request::Status).unwrap(), None);
        serve(|request| Response {
            status: Some(serde_json::json!({ "on": matches!(request, Request::On { .. }) })),
            ..Response::default()
        })
        .unwrap();
        let response = send(&Request::On { nudge: false }).unwrap().unwrap();
        assert_eq!(response.status, Some(serde_json::json!({ "on": true })));
        let mode = std::fs::metadata(dir.join("unidle.sock")).unwrap();
        assert_eq!(
            std::os::unix::fs::PermissionsExt::mode(&mode.permissions()) & 0o777,
            0o600
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
