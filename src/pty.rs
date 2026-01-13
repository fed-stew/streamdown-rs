//! PTY (pseudo-terminal) handling for subprocess wrapping.
//!
//! This module provides PTY functionality for the `--exec` feature,
//! allowing streamdown to wrap interactive programs and render their
//! output as markdown while forwarding keyboard input.

use std::io;
use std::time::Duration;

/// Result of polling for input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollResult {
    /// Data available on stdin (keyboard input)
    Stdin,
    /// Data available from subprocess
    Master,
    /// Both stdin and master have data
    Both,
    /// Timeout expired with no data
    Timeout,
    /// An error occurred
    Error,
}

/// Check if PTY support is available on this platform.
pub fn is_supported() -> bool {
    cfg!(unix)
}

/// Return an error message for unsupported platforms.
pub fn unsupported_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "PTY support (--exec) is not available on this platform",
    )
}

// ============================================================================
// Unix Implementation
// ============================================================================

#[cfg(unix)]
mod unix {
    use super::*;
    use nix::libc;
    use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
    use nix::pty::{openpty, OpenptyResult};
    use nix::sys::termios::{self, LocalFlags, SetArg, Termios};
    use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
    use nix::unistd::{dup2, fork, read, write, ForkResult, Pid};
    use std::ffi::CString;
    use std::io::Write;
    use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};

    /// A PTY session wrapping a subprocess.
    pub struct PtySession {
        /// Master side of the PTY (owned)
        master: OwnedFd,
        /// Child process ID
        child_pid: Pid,
        /// Original terminal settings (for restoration)
        original_termios: Option<Termios>,
        /// Count of keyboard bytes sent (for echo handling)
        keyboard_count: usize,
        /// Whether the child is still running
        child_alive: bool,
    }

    impl PtySession {
        /// Create a new PTY session wrapping the given command.
        pub fn spawn(command: &str) -> io::Result<Self> {
            // Parse command into program and arguments
            let parts: Vec<&str> = command.split_whitespace().collect();
            if parts.is_empty() {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "Empty command"));
            }

            // Save original terminal settings
            // SAFETY: STDIN_FILENO is always valid for the process lifetime
            let stdin_fd = unsafe { BorrowedFd::borrow_raw(libc::STDIN_FILENO) };
            let original_termios = termios::tcgetattr(stdin_fd).ok();

            // Open PTY pair
            let OpenptyResult { master, slave } =
                openpty(None, None).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            // Fork
            match unsafe { fork() } {
                Ok(ForkResult::Child) => {
                    // Child process
                    // Close master in child
                    drop(master);

                    // Create new session and set controlling terminal
                    let _ = nix::unistd::setsid();

                    // Get raw fd from slave
                    let slave_fd = slave.as_raw_fd();

                    // Duplicate slave to stdin/stdout/stderr
                    let _ = dup2(slave_fd, libc::STDIN_FILENO);
                    let _ = dup2(slave_fd, libc::STDOUT_FILENO);
                    let _ = dup2(slave_fd, libc::STDERR_FILENO);

                    // Close original slave fd if it's not one of the standard fds
                    if slave_fd > libc::STDERR_FILENO {
                        drop(slave);
                    }

                    // Execute command - handle null bytes safely in child process
                    let program = match CString::new(parts[0]) {
                        Ok(c) => c,
                        Err(_) => std::process::exit(127),
                    };
                    let args: Vec<CString> =
                        parts.iter().filter_map(|s| CString::new(*s).ok()).collect();

                    // This doesn't return on success
                    let _ = nix::unistd::execvp(&program, &args);

                    // If we get here, exec failed
                    std::process::exit(127);
                }
                Ok(ForkResult::Parent { child }) => {
                    // Parent process
                    // Close slave in parent
                    drop(slave);

                    // Set stdin to cbreak mode (raw, no echo, no line buffering)
                    if let Some(ref orig) = original_termios {
                        let mut raw = orig.clone();
                        // Disable canonical mode and echo
                        raw.local_flags.remove(LocalFlags::ICANON);
                        raw.local_flags.remove(LocalFlags::ECHO);
                        raw.local_flags.remove(LocalFlags::ISIG);
                        // Set minimum chars and timeout
                        raw.control_chars[libc::VMIN] = 1;
                        raw.control_chars[libc::VTIME] = 0;

                        let _ = termios::tcsetattr(stdin_fd, SetArg::TCSANOW, &raw);
                    }

                    // Enable auto-wrap
                    let _ = io::stdout().write_all(b"\x1b[?7h");
                    let _ = io::stdout().flush();

                    // Set master to non-blocking using libc directly
                    unsafe {
                        let flags = libc::fcntl(master.as_raw_fd(), libc::F_GETFL);
                        libc::fcntl(master.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK);
                    }

                    Ok(PtySession {
                        master,
                        child_pid: child,
                        original_termios,
                        keyboard_count: 0,
                        child_alive: true,
                    })
                }
                Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
            }
        }

        /// Poll for available input with a timeout.
        pub fn poll(&self, timeout: Duration) -> PollResult {
            let stdin_fd = libc::STDIN_FILENO;
            let _master_fd = self.master.as_raw_fd();

            let mut fds = [
                PollFd::new(
                    unsafe { BorrowedFd::borrow_raw(stdin_fd) },
                    PollFlags::POLLIN,
                ),
                PollFd::new(self.master.as_fd(), PollFlags::POLLIN),
            ];

            let timeout_ms = timeout.as_millis() as i32;
            let poll_timeout = PollTimeout::try_from(timeout_ms).unwrap_or(PollTimeout::ZERO);

            match poll(&mut fds, poll_timeout) {
                Ok(0) => PollResult::Timeout,
                Ok(_) => {
                    let stdin_ready = fds[0]
                        .revents()
                        .map(|r| r.contains(PollFlags::POLLIN))
                        .unwrap_or(false);
                    let master_ready = fds[1]
                        .revents()
                        .map(|r| r.contains(PollFlags::POLLIN))
                        .unwrap_or(false);

                    match (stdin_ready, master_ready) {
                        (true, true) => PollResult::Both,
                        (true, false) => PollResult::Stdin,
                        (false, true) => PollResult::Master,
                        (false, false) => PollResult::Timeout,
                    }
                }
                Err(_) => PollResult::Error,
            }
        }

        /// Read bytes from the master (subprocess output).
        pub fn read_master(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match read(self.master.as_raw_fd(), buf) {
                Ok(n) => Ok(n),
                Err(nix::errno::Errno::EAGAIN) => Ok(0),
                Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
            }
        }

        /// Read a single byte from master.
        #[allow(dead_code)]
        pub fn read_master_byte(&mut self) -> io::Result<Option<u8>> {
            let mut buf = [0u8; 1];
            match self.read_master(&mut buf)? {
                0 => Ok(None),
                _ => Ok(Some(buf[0])),
            }
        }

        /// Write bytes to the master (keyboard input to subprocess).
        pub fn write_master(&mut self, data: &[u8]) -> io::Result<usize> {
            self.keyboard_count += data.len();
            write(&self.master, data).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
        }

        /// Write a single byte to master.
        pub fn write_master_byte(&mut self, byte: u8) -> io::Result<()> {
            self.write_master(&[byte])?;
            Ok(())
        }

        /// Read a byte from stdin.
        pub fn read_stdin_byte(&self) -> io::Result<Option<u8>> {
            let mut buf = [0u8; 1];
            match read(libc::STDIN_FILENO, &mut buf) {
                Ok(0) => Ok(None),
                Ok(_) => Ok(Some(buf[0])),
                Err(nix::errno::Errno::EAGAIN) => Ok(None),
                Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
            }
        }

        /// Get the keyboard byte count (for echo handling).
        pub fn keyboard_count(&self) -> usize {
            self.keyboard_count
        }

        /// Reset keyboard count (after newline).
        pub fn reset_keyboard_count(&mut self) {
            self.keyboard_count = 0;
        }

        /// Check if the child process is still running.
        pub fn is_alive(&mut self) -> bool {
            if !self.child_alive {
                return false;
            }

            match waitpid(self.child_pid, Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::StillAlive) => true,
                Ok(_) => {
                    self.child_alive = false;
                    false
                }
                Err(_) => {
                    self.child_alive = false;
                    false
                }
            }
        }

        /// Wait for the child process to exit.
        pub fn wait(&mut self) -> io::Result<i32> {
            match waitpid(self.child_pid, None) {
                Ok(WaitStatus::Exited(_, code)) => {
                    self.child_alive = false;
                    Ok(code)
                }
                Ok(WaitStatus::Signaled(_, signal, _)) => {
                    self.child_alive = false;
                    Ok(128 + signal as i32)
                }
                Ok(_) => Ok(0),
                Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
            }
        }

        /// Get the master file descriptor.
        #[allow(dead_code)]
        pub fn master_fd(&self) -> RawFd {
            self.master.as_raw_fd()
        }
    }

    impl Drop for PtySession {
        fn drop(&mut self) {
            // Restore original terminal settings
            if let Some(ref orig) = self.original_termios {
                // SAFETY: STDIN_FILENO is always valid for the process lifetime
                let stdin_fd = unsafe { BorrowedFd::borrow_raw(libc::STDIN_FILENO) };
                let _ = termios::tcsetattr(stdin_fd, SetArg::TCSADRAIN, orig);
            }

            // Wait for child if still alive
            if self.child_alive {
                let _ = waitpid(self.child_pid, None);
            }
        }
    }
}

// Re-export Unix implementation
#[cfg(unix)]
pub use unix::PtySession;

// ============================================================================
// Windows Stub
// ============================================================================

#[cfg(windows)]
pub struct PtySession;

#[cfg(windows)]
impl PtySession {
    pub fn spawn(_command: &str) -> io::Result<Self> {
        Err(unsupported_error())
    }

    pub fn poll(&self, _timeout: Duration) -> PollResult {
        PollResult::Error
    }

    pub fn read_master(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Err(unsupported_error())
    }

    pub fn read_master_byte(&mut self) -> io::Result<Option<u8>> {
        Err(unsupported_error())
    }

    pub fn write_master(&mut self, _data: &[u8]) -> io::Result<usize> {
        Err(unsupported_error())
    }

    pub fn write_master_byte(&mut self, _byte: u8) -> io::Result<()> {
        Err(unsupported_error())
    }

    pub fn read_stdin_byte(&self) -> io::Result<Option<u8>> {
        Err(unsupported_error())
    }

    pub fn keyboard_count(&self) -> usize {
        0
    }

    pub fn reset_keyboard_count(&mut self) {}

    pub fn is_alive(&mut self) -> bool {
        false
    }

    pub fn wait(&mut self) -> io::Result<i32> {
        Err(unsupported_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported() {
        #[cfg(unix)]
        assert!(is_supported());

        #[cfg(windows)]
        assert!(!is_supported());
    }

    #[test]
    fn test_poll_result_eq() {
        assert_eq!(PollResult::Stdin, PollResult::Stdin);
        assert_ne!(PollResult::Stdin, PollResult::Master);
    }
}
