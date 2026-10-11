use std::ffi::c_void;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

use windows::core::{Result, PCWSTR, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows::Win32::System::Console::{
    ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON,
};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
    UpdateProcThreadAttribute, EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, STARTUPINFOEXW,
};

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn strip_vt(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            if ch != '\r' {
                result.push(ch);
            }
            continue;
        }
        if chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
        }
    }
    result
}

pub struct ConPtySession {
    pseudo_console: HPCON,
    input: HANDLE,
    process: HANDLE,
    pub lines: Arc<Mutex<Vec<String>>>,
}

impl ConPtySession {
    pub unsafe fn spawn(cwd: &Path, columns: i16, rows: i16) -> Result<Self> {
        let mut pseudo_input_read = HANDLE::default();
        let mut input_write = HANDLE::default();
        let mut output_read = HANDLE::default();
        let mut pseudo_output_write = HANDLE::default();
        unsafe {
            CreatePipe(&mut pseudo_input_read, &mut input_write, None, 0)?;
            CreatePipe(&mut output_read, &mut pseudo_output_write, None, 0)?;
        }
        let pseudo_console = match unsafe {
            CreatePseudoConsole(
                COORD {
                    X: columns.max(20),
                    Y: rows.max(4),
                },
                pseudo_input_read,
                pseudo_output_write,
                0,
            )
        } {
            Ok(value) => value,
            Err(error) => {
                let _ = unsafe { CloseHandle(pseudo_input_read) };
                let _ = unsafe { CloseHandle(input_write) };
                let _ = unsafe { CloseHandle(output_read) };
                let _ = unsafe { CloseHandle(pseudo_output_write) };
                return Err(error);
            }
        };
        let _ = unsafe { CloseHandle(pseudo_input_read) };
        let _ = unsafe { CloseHandle(pseudo_output_write) };

        let mut attribute_bytes = 0usize;
        let _ = unsafe { InitializeProcThreadAttributeList(None, 1, None, &mut attribute_bytes) };
        let word_count =
            (attribute_bytes + std::mem::size_of::<usize>() - 1) / std::mem::size_of::<usize>();
        let mut attribute_storage = vec![0usize; word_count.max(1)];
        let attribute_list =
            LPPROC_THREAD_ATTRIBUTE_LIST(attribute_storage.as_mut_ptr().cast::<c_void>());
        unsafe {
            InitializeProcThreadAttributeList(Some(attribute_list), 1, None, &mut attribute_bytes)?;
            UpdateProcThreadAttribute(
                attribute_list,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                Some((&pseudo_console as *const HPCON).cast::<c_void>()),
                std::mem::size_of::<HPCON>(),
                None,
                None,
            )?;
        }
        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        startup.lpAttributeList = attribute_list;
        let mut command = wide("powershell.exe -NoLogo");
        let cwd = wide(&cwd.to_string_lossy());
        let mut process_info = PROCESS_INFORMATION::default();
        let process_result = unsafe {
            CreateProcessW(
                PCWSTR::null(),
                Some(PWSTR(command.as_mut_ptr())),
                None,
                None,
                false,
                EXTENDED_STARTUPINFO_PRESENT,
                None,
                PCWSTR(cwd.as_ptr()),
                &startup.StartupInfo,
                &mut process_info,
            )
        };
        unsafe { DeleteProcThreadAttributeList(attribute_list) };
        if let Err(error) = process_result {
            unsafe { ClosePseudoConsole(pseudo_console) };
            let _ = unsafe { CloseHandle(input_write) };
            let _ = unsafe { CloseHandle(output_read) };
            return Err(error);
        }
        let _ = unsafe { CloseHandle(process_info.hThread) };

        let lines = Arc::new(Mutex::new(vec![
            "TianChuangYa  /  TC-IDE native ConPTY".into(),
            "GitHub: https://github.com/tianchuangya/TC-IDE".into(),
        ]));
        let output_lines = Arc::clone(&lines);
        let output_read_raw = output_read.0 as usize;
        thread::spawn(move || {
            let output_read = HANDLE(output_read_raw as *mut c_void);
            let mut buffer = [0u8; 4096];
            let mut pending = String::new();
            loop {
                let mut read = 0u32;
                if unsafe { ReadFile(output_read, Some(&mut buffer), Some(&mut read), None) }
                    .is_err()
                    || read == 0
                {
                    break;
                }
                pending.push_str(&String::from_utf8_lossy(&buffer[..read as usize]));
                let clean = strip_vt(&pending);
                let complete = clean.ends_with('\n');
                let mut pieces = clean.split('\n').map(str::to_string).collect::<Vec<_>>();
                pending = if complete {
                    String::new()
                } else {
                    pieces.pop().unwrap_or_default()
                };
                if let Ok(mut target) = output_lines.lock() {
                    target.extend(pieces.into_iter().filter(|line| !line.is_empty()));
                    if target.len() > 800 {
                        let remove = target.len() - 800;
                        target.drain(..remove);
                    }
                }
            }
            let _ = unsafe { CloseHandle(output_read) };
        });

        Ok(Self {
            pseudo_console,
            input: input_write,
            process: process_info.hProcess,
            lines,
        })
    }

    pub fn write(&self, value: &str) -> Result<()> {
        let bytes = value.as_bytes();
        let mut written = 0u32;
        unsafe { WriteFile(self.input, Some(bytes), Some(&mut written), None) }
    }

    pub fn resize(&self, columns: i16, rows: i16) {
        let _ = unsafe {
            ResizePseudoConsole(
                self.pseudo_console,
                COORD {
                    X: columns.max(20),
                    Y: rows.max(4),
                },
            )
        };
    }
}

impl Drop for ConPtySession {
    fn drop(&mut self) {
        unsafe { ClosePseudoConsole(self.pseudo_console) };
        let _ = unsafe { CloseHandle(self.input) };
        let _ = unsafe { CloseHandle(self.process) };
    }
}

#[cfg(test)]
mod tests {
    use super::strip_vt;

    #[test]
    fn strips_common_virtual_terminal_sequences() {
        assert_eq!(strip_vt("\u{1b}[31mred\u{1b}[0m\r\n"), "red\n");
    }
}
