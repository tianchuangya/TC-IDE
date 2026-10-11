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

#[derive(Default)]
pub struct TerminalView {
    pub lines: Vec<String>,
    pub current: String,
}

enum EscapeState {
    Text,
    Escape,
    Csi(String),
    Osc,
    OscEscape,
}

struct TerminalParser {
    lines: Vec<String>,
    current: Vec<char>,
    cursor: usize,
    state: EscapeState,
}

impl TerminalParser {
    fn new() -> Self {
        Self {
            lines: vec![
                "TianChuangYa  /  TC-IDE native ConPTY".into(),
                "GitHub: https://github.com/tianchuangya/TC-IDE".into(),
            ],
            current: Vec::new(),
            cursor: 0,
            state: EscapeState::Text,
        }
    }

    fn feed(&mut self, value: &str) {
        for ch in value.chars() {
            let state = std::mem::replace(&mut self.state, EscapeState::Text);
            self.state = match state {
                EscapeState::Text => self.feed_text(ch),
                EscapeState::Escape if ch == '[' => EscapeState::Csi(String::new()),
                EscapeState::Escape if ch == ']' => EscapeState::Osc,
                EscapeState::Escape => EscapeState::Text,
                EscapeState::Csi(mut sequence) => {
                    if ('@'..='~').contains(&ch) {
                        self.apply_csi(&sequence, ch);
                        EscapeState::Text
                    } else {
                        sequence.push(ch);
                        EscapeState::Csi(sequence)
                    }
                }
                EscapeState::Osc if ch == '\u{7}' => EscapeState::Text,
                EscapeState::Osc if ch == '\u{1b}' => EscapeState::OscEscape,
                EscapeState::Osc => EscapeState::Osc,
                EscapeState::OscEscape if ch == '\\' => EscapeState::Text,
                EscapeState::OscEscape => EscapeState::Osc,
            };
        }
    }

    fn feed_text(&mut self, ch: char) -> EscapeState {
        match ch {
            '\u{1b}' => return EscapeState::Escape,
            '\r' => self.cursor = 0,
            '\n' => {
                self.lines.push(self.current.iter().collect());
                self.current.clear();
                self.cursor = 0;
                if self.lines.len() > 800 {
                    let remove = self.lines.len() - 800;
                    self.lines.drain(..remove);
                }
            }
            '\u{8}' => self.cursor = self.cursor.saturating_sub(1),
            value if !value.is_control() => {
                while self.current.len() < self.cursor {
                    self.current.push(' ');
                }
                if self.cursor < self.current.len() {
                    self.current[self.cursor] = value;
                } else {
                    self.current.push(value);
                }
                self.cursor += 1;
            }
            _ => {}
        }
        EscapeState::Text
    }

    fn apply_csi(&mut self, sequence: &str, command: char) {
        let params = sequence
            .trim_start_matches('?')
            .split(';')
            .map(|value| value.parse::<usize>().unwrap_or(0))
            .collect::<Vec<_>>();
        let first = params.first().copied().unwrap_or(0);
        match command {
            'C' => self.cursor = self.cursor.saturating_add(first.max(1)),
            'D' => self.cursor = self.cursor.saturating_sub(first.max(1)),
            'G' => self.cursor = first.max(1) - 1,
            'K' => match first {
                1 => {
                    for value in self.current.iter_mut().take(self.cursor.saturating_add(1)) {
                        *value = ' ';
                    }
                }
                2 => {
                    self.current.clear();
                    self.cursor = 0;
                }
                _ => self.current.truncate(self.cursor),
            },
            'J' if first == 2 || first == 3 => {
                self.lines.clear();
                self.current.clear();
                self.cursor = 0;
            }
            _ => {}
        }
    }

    fn publish(&self, target: &Arc<Mutex<TerminalView>>) {
        if let Ok(mut view) = target.lock() {
            view.lines.clone_from(&self.lines);
            view.current = self.current.iter().collect();
        }
    }
}

pub struct ConPtySession {
    pseudo_console: HPCON,
    input: HANDLE,
    process: HANDLE,
    pub view: Arc<Mutex<TerminalView>>,
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

        let view = Arc::new(Mutex::new(TerminalView::default()));
        let output_view = Arc::clone(&view);
        let output_read_raw = output_read.0 as usize;
        thread::spawn(move || {
            let output_read = HANDLE(output_read_raw as *mut c_void);
            let mut buffer = [0u8; 4096];
            let mut parser = TerminalParser::new();
            parser.publish(&output_view);
            loop {
                let mut read = 0u32;
                if unsafe { ReadFile(output_read, Some(&mut buffer), Some(&mut read), None) }
                    .is_err()
                    || read == 0
                {
                    break;
                }
                parser.feed(&String::from_utf8_lossy(&buffer[..read as usize]));
                parser.publish(&output_view);
            }
            let _ = unsafe { CloseHandle(output_read) };
        });

        Ok(Self {
            pseudo_console,
            input: input_write,
            process: process_info.hProcess,
            view,
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
    use super::TerminalParser;

    #[test]
    fn parses_virtual_terminal_sequences_and_live_prompt() {
        let mut parser = TerminalParser::new();
        parser.feed("\u{1b}[31mred\u{1b}[0m\r\nPS C:\\\\repo> ");
        assert_eq!(parser.lines.last().map(String::as_str), Some("red"));
        assert_eq!(parser.current.iter().collect::<String>(), "PS C:\\\\repo> ");
    }

    #[test]
    fn carriage_return_overwrites_the_current_line() {
        let mut parser = TerminalParser::new();
        parser.feed("progress 10%\rprogress 90%\u{1b}[K");
        assert_eq!(parser.current.iter().collect::<String>(), "progress 90%");
    }
}
