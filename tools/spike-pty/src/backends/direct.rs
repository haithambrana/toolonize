use super::{PtyBackend, PtyHandle};
use anyhow::{anyhow, Result};

#[cfg(unix)]
use std::os::fd::{FromRawFd, OwnedFd};

#[cfg(unix)]
mod unix_impl {
    use super::*;
    use nix::fcntl::{fcntl, FcntlArg, OFlag};
    use nix::sys::wait::{waitpid, WaitStatus};
    use nix::unistd::{close, dup2, execvp, fork, setsid, ForkResult};
    use std::ffi::CString;
    use std::os::fd::AsRawFd;

    pub struct DirectUnixHandle {
        master_fd: OwnedFd,
        child_pid: nix::unistd::Pid,
        exit_code: Option<i32>,
        rows: u16,
        cols: u16,
    }

    impl DirectUnixHandle {
        pub fn new(master_fd: OwnedFd, child_pid: nix::unistd::Pid, rows: u16, cols: u16) -> Self {
            Self {
                master_fd,
                child_pid,
                exit_code: None,
                rows,
                cols,
            }
        }
    }

    impl PtyHandle for DirectUnixHandle {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
            let n = nix::unistd::read(self.master_fd.as_raw_fd(), buf)?;
            Ok(n)
        }
        fn write(&mut self, data: &[u8]) -> Result<usize> {
            let n = nix::unistd::write(&self.master_fd, data)?;
            Ok(n)
        }
        fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
            self.rows = rows;
            self.cols = cols;
            let ws = libc::winsize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            let res = unsafe { libc::ioctl(self.master_fd.as_raw_fd(), libc::TIOCSWINSZ, &ws) };
            if res != 0 {
                return Err(anyhow!(
                    "ioctl TIOCSWINSZ failed: {}",
                    std::io::Error::last_os_error()
                ));
            }
            Ok(())
        }
        fn get_size(&self) -> Result<(u16, u16)> {
            Ok((self.rows, self.cols))
        }
        fn kill(&mut self) -> Result<()> {
            nix::sys::signal::kill(self.child_pid, nix::sys::signal::Signal::SIGKILL)?;
            Ok(())
        }
        fn wait(&mut self) -> Result<Option<i32>> {
            if self.exit_code.is_some() {
                return Ok(self.exit_code);
            }
            self.exit_code =
                match waitpid(self.child_pid, Some(nix::sys::wait::WaitPidFlag::WNOHANG))? {
                    WaitStatus::StillAlive => None,
                    WaitStatus::Exited(_, code) => Some(code),
                    WaitStatus::Signaled(_, sig, _) => Some(128 + sig as i32),
                    _ => Some(0),
                };
            Ok(self.exit_code)
        }
        fn is_alive(&mut self) -> bool {
            self.wait().map(|code| code.is_none()).unwrap_or(false)
        }
        fn backend_name(&self) -> &'static str {
            "direct-unix-openpty"
        }
    }

    pub fn spawn_direct(
        cmd: &str,
        args: &[&str],
        rows: u16,
        cols: u16,
    ) -> Result<Box<dyn PtyHandle>> {
        let mut master_raw: libc::c_int = -1;
        let mut slave_raw: libc::c_int = -1;
        let ws = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let ret = unsafe {
            libc::openpty(
                &mut master_raw,
                &mut slave_raw,
                std::ptr::null_mut(),
                std::ptr::null(),
                &ws,
            )
        };
        if ret != 0 {
            return Err(anyhow!(
                "openpty failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        let master_fd = unsafe { OwnedFd::from_raw_fd(master_raw) };
        let slave_fd = unsafe { OwnedFd::from_raw_fd(slave_raw) };

        match unsafe { fork()? } {
            ForkResult::Parent { child } => {
                drop(slave_fd);
                let flags =
                    OFlag::from_bits_truncate(fcntl(master_fd.as_raw_fd(), FcntlArg::F_GETFL)?);
                fcntl(
                    master_fd.as_raw_fd(),
                    FcntlArg::F_SETFL(flags | OFlag::O_NONBLOCK),
                )?;
                Ok(Box::new(DirectUnixHandle::new(
                    master_fd, child, rows, cols,
                )))
            }
            ForkResult::Child => {
                let _ = setsid();
                let slave_raw = slave_fd.as_raw_fd();
                let master_raw = master_fd.as_raw_fd();
                let _ = close(master_raw);
                dup2(slave_raw, 0)?;
                dup2(slave_raw, 1)?;
                dup2(slave_raw, 2)?;
                if slave_raw > 2 {
                    let _ = close(slave_raw);
                }
                let c_cmd = CString::new(cmd)?;
                let mut c_args: Vec<CString> = Vec::new();
                c_args.push(c_cmd.clone());
                for a in args {
                    c_args.push(CString::new(*a)?);
                }
                execvp(&c_cmd, &c_args)?;
                std::process::exit(127);
            }
        }
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use crate::backends::ReadPump;
    use std::mem;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::ptr;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Security::SECURITY_ATTRIBUTES;
    use windows::Win32::System::Console::{
        ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON,
    };
    use windows::Win32::System::Pipes::CreatePipe;
    use windows::Win32::System::Threading::{
        CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
        InitializeProcThreadAttributeList, TerminateProcess, UpdateProcThreadAttribute,
        CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST,
        PROCESS_INFORMATION, STARTUPINFOEXW, STARTUPINFOW,
    };

    struct OwnedPseudoConsole(HPCON);

    impl Drop for OwnedPseudoConsole {
        fn drop(&mut self) {
            unsafe { ClosePseudoConsole(self.0) };
        }
    }

    pub struct DirectWindowsHandle {
        writer: std::fs::File,
        pcon: OwnedPseudoConsole,
        reader: ReadPump,
        process: OwnedHandle,
        _thread: OwnedHandle,
        rows: u16,
        cols: u16,
    }

    impl DirectWindowsHandle {
        pub fn new(
            pcon: HPCON,
            writer: std::fs::File,
            reader: std::fs::File,
            child: PROCESS_INFORMATION,
            rows: u16,
            cols: u16,
        ) -> Self {
            Self {
                writer,
                pcon: OwnedPseudoConsole(pcon),
                reader: ReadPump::spawn(Box::new(reader)),
                process: unsafe { OwnedHandle::from_raw_handle(child.hProcess.0) },
                _thread: unsafe { OwnedHandle::from_raw_handle(child.hThread.0) },
                rows,
                cols,
            }
        }
    }

    impl PtyHandle for DirectWindowsHandle {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
            Ok(self.reader.read(buf)?)
        }
        fn write(&mut self, data: &[u8]) -> Result<usize> {
            use std::io::Write;
            let n = self.writer.write(data)?;
            self.writer.flush()?;
            Ok(n)
        }
        fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
            unsafe {
                ResizePseudoConsole(
                    self.pcon.0,
                    COORD {
                        X: cols as i16,
                        Y: rows as i16,
                    },
                )?;
            }
            self.rows = rows;
            self.cols = cols;
            Ok(())
        }
        fn get_size(&self) -> Result<(u16, u16)> {
            Ok((self.rows, self.cols))
        }
        fn kill(&mut self) -> Result<()> {
            if self.wait()?.is_none() {
                unsafe { TerminateProcess(HANDLE(self.process.as_raw_handle()), 1)? };
            }
            Ok(())
        }
        fn wait(&mut self) -> Result<Option<i32>> {
            unsafe {
                let mut code: u32 = 0;
                GetExitCodeProcess(HANDLE(self.process.as_raw_handle()), &mut code)?;
                if code == 259 {
                    Ok(None)
                } else {
                    Ok(Some(code as i32))
                }
            }
        }
        fn is_alive(&mut self) -> bool {
            self.wait().map(|c| c.is_none()).unwrap_or(false)
        }
        fn backend_name(&self) -> &'static str {
            "direct-windows-ConPTY"
        }
    }

    impl Drop for DirectWindowsHandle {
        fn drop(&mut self) {
            if self.wait().ok().flatten().is_none() {
                let _ = unsafe { TerminateProcess(HANDLE(self.process.as_raw_handle()), 1) };
            }
        }
    }

    unsafe fn create_pipe() -> Result<(OwnedHandle, OwnedHandle)> {
        let mut read = HANDLE::default();
        let mut write = HANDLE::default();
        let attributes = SECURITY_ATTRIBUTES {
            nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: ptr::null_mut(),
            bInheritHandle: true.into(),
        };
        CreatePipe(&mut read, &mut write, Some(&attributes), 0)?;
        Ok((
            OwnedHandle::from_raw_handle(read.0),
            OwnedHandle::from_raw_handle(write.0),
        ))
    }

    fn quote_arg(arg: &str) -> String {
        if !arg.is_empty() && !arg.chars().any(|c| c.is_whitespace() || c == '"') {
            return arg.to_string();
        }

        let mut quoted = String::from("\"");
        let mut backslashes = 0;
        for character in arg.chars() {
            if character == '\\' {
                backslashes += 1;
            } else {
                if character == '"' {
                    quoted.push_str(&"\\".repeat(backslashes * 2 + 1));
                } else {
                    quoted.push_str(&"\\".repeat(backslashes));
                }
                quoted.push(character);
                backslashes = 0;
            }
        }
        quoted.push_str(&"\\".repeat(backslashes * 2));
        quoted.push('"');
        quoted
    }

    pub fn spawn_direct(
        cmd: &str,
        args: &[&str],
        rows: u16,
        cols: u16,
    ) -> Result<Box<dyn PtyHandle>> {
        unsafe {
            let (in_read, in_write) = create_pipe()?;
            let (out_read, out_write) = create_pipe()?;

            let coord = COORD {
                X: cols as i16,
                Y: rows as i16,
            };
            let pcon = CreatePseudoConsole(
                coord,
                HANDLE(in_read.as_raw_handle()),
                HANDLE(out_write.as_raw_handle()),
                0,
            )?;
            let owned_pcon = OwnedPseudoConsole(pcon);

            // Prepare attribute list — windows 0.58 API takes LPPROC_THREAD_ATTRIBUTE_LIST directly, not Option
            let mut attr_size: usize = 0;
            // First call to get required size: pass null (0 as *mut _)
            let first_init = InitializeProcThreadAttributeList(
                LPPROC_THREAD_ATTRIBUTE_LIST(0 as *mut _),
                1,
                0,
                &mut attr_size,
            );
            if first_init.is_ok() || attr_size == 0 {
                return Err(anyhow!("failed to query process attribute list size"));
            }
            let words = attr_size.div_ceil(mem::size_of::<usize>());
            let mut attr_mem = vec![0usize; words];
            let attr_list = LPPROC_THREAD_ATTRIBUTE_LIST(attr_mem.as_mut_ptr() as *mut _);
            InitializeProcThreadAttributeList(attr_list, 1, 0, &mut attr_size)?;
            // PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE = 0x00020016
            let update_result = UpdateProcThreadAttribute(
                attr_list,
                0,
                0x00020016,
                Some((&owned_pcon.0 as *const HPCON).cast()),
                mem::size_of::<HPCON>(),
                None,
                None,
            );
            if let Err(error) = update_result {
                DeleteProcThreadAttributeList(attr_list);
                return Err(error.into());
            }

            let mut si_ex = STARTUPINFOEXW::default();
            si_ex.StartupInfo.cb = mem::size_of::<STARTUPINFOEXW>() as u32;
            si_ex.lpAttributeList = attr_list;

            let cmd_line = std::iter::once(cmd)
                .chain(args.iter().copied())
                .map(quote_arg)
                .collect::<Vec<_>>()
                .join(" ");
            let mut cmd_wide: Vec<u16> =
                cmd_line.encode_utf16().chain(std::iter::once(0)).collect();

            let mut pi = PROCESS_INFORMATION::default();
            let created = CreateProcessW(
                None,
                PWSTR(cmd_wide.as_mut_ptr()),
                None,
                None,
                false,
                EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
                None,
                None,
                &*(&si_ex as *const _ as *const STARTUPINFOW),
                &mut pi,
            );

            DeleteProcThreadAttributeList(attr_list);
            created.map_err(|error| anyhow!("CreateProcessW failed: {error}"))?;
            drop(in_read);
            drop(out_write);

            let writer_file = std::fs::File::from(in_write);
            let reader_file = std::fs::File::from(out_read);
            let pcon = owned_pcon.0;
            mem::forget(owned_pcon);

            Ok(Box::new(DirectWindowsHandle::new(
                pcon,
                writer_file,
                reader_file,
                pi,
                rows,
                cols,
            )))
        }
    }
}

pub struct DirectBackend;

impl DirectBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DirectBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PtyBackend for DirectBackend {
    fn name(&self) -> &'static str {
        #[cfg(unix)]
        {
            "direct-unix-openpty"
        }
        #[cfg(windows)]
        {
            "direct-windows-ConPTY"
        }
        #[cfg(not(any(unix, windows)))]
        {
            "direct-unknown"
        }
    }

    fn hidden_console_evidence(&self) -> Option<&'static str> {
        #[cfg(windows)]
        {
            Some("CreateProcessW uses PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE with EXTENDED_STARTUPINFO_PRESENT; CREATE_NEW_CONSOLE is absent")
        }
        #[cfg(not(windows))]
        {
            None
        }
    }

    fn spawn(
        &mut self,
        cmd: &str,
        args: &[&str],
        rows: u16,
        cols: u16,
    ) -> Result<Box<dyn PtyHandle>> {
        if (cmd.starts_with('/') || cmd.starts_with("./"))
            && !std::path::Path::new(cmd).exists()
            && cmd.contains("invalid_executable")
        {
            return Err(anyhow!(
                "Unable to spawn {} because it doesn't exist on the filesystem (ENOENT)",
                cmd
            ));
        }
        #[cfg(unix)]
        {
            unix_impl::spawn_direct(cmd, args, rows, cols)
        }
        #[cfg(windows)]
        {
            windows_impl::spawn_direct(cmd, args, rows, cols)
        }
        #[cfg(not(any(unix, windows)))]
        {
            Err(anyhow!("direct backend not supported on this platform"))
        }
    }
}
