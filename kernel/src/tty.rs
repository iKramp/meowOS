use std::{boxed::Box, format, lock_w_info, string::{String, ToString}, sync::no_int_spinlock::NoIntSpinlock, vec::Vec};

use crate::{
    keyboard::{self, Key},
    proc::{self, Pid},
    vfs, vga::vga_text,
};

pub static TTY: NoIntSpinlock<TtyState> = NoIntSpinlock::new(TtyState::new());

pub struct TtyState {
    done_streams: Vec<(String, bool)>, //(stream, ends with EOF)
    input_buffer: String,
    running_pid: Option<Pid>,
    started_proc: bool,
}

impl TtyState {
    const fn new() -> Self {
        Self {
            done_streams: Vec::new(),
            input_buffer: String::new(),
            running_pid: None,
            started_proc: false,
        }
    }

    pub fn data_len(&self) -> usize {
        self.done_streams.iter().map(|(s, _)| s.len()).sum::<usize>()
    }

    pub fn get_input(&mut self, max_size: u64) -> Option<String> {
        if let Some((stream, _)) = self.done_streams.pop() {
            if stream.len() as u64 > max_size {
                let mut split_point = max_size as usize;
                while !stream.is_char_boundary(split_point) {
                    split_point -= 1;
                }
                let remaining = stream[split_point..].to_string();
                self.done_streams.insert(0, (remaining, false));
                Some(stream[..split_point].to_string())
            } else {
                Some(stream)
            }
        } else {
            None
        }
    }

    pub fn handle_input(&mut self, input: Key, event: keyboard::KeyEvent, modifier_state: &keyboard::KeyboardState) {
        if event == keyboard::KeyEvent::Released {
            return;
        }

        if modifier_state.ctrl() {
            if modifier_state.shift() {
                return; //ctrl + shift is not a valid combination
            }
            //special keys
            match input {
                Key::C => {
                    //TODO: signals
                    if let Some(pid) = self.running_pid {
                        proc::kill_process(pid, 0);
                        self.running_pid = None;
                        self.started_proc = false;
                    }
                }
                Key::D => {
                    self.send_line(true);
                }
                _ => {}
            }
        }

        let buf_to_fill_size_before = self.input_buffer.len();

        if !modifier_state.shift() {
            match input {
                Key::Backtick => self.input_buffer.push('`'),
                Key::K1 | Key::Num1 => self.input_buffer.push('1'),
                Key::K2 | Key::Num2 => self.input_buffer.push('2'),
                Key::K3 | Key::Num3 => self.input_buffer.push('3'),
                Key::K4 | Key::Num4 => self.input_buffer.push('4'),
                Key::K5 | Key::Num5 => self.input_buffer.push('5'),
                Key::K6 | Key::Num6 => self.input_buffer.push('6'),
                Key::K7 | Key::Num7 => self.input_buffer.push('7'),
                Key::K8 | Key::Num8 => self.input_buffer.push('8'),
                Key::K9 | Key::Num9 => self.input_buffer.push('9'),
                Key::K0 | Key::Num0 => self.input_buffer.push('0'),
                Key::Dash => self.input_buffer.push('-'),
                Key::Equal => self.input_buffer.push('='),
                Key::Backspace => {
                    self.input_buffer.pop();
                    lock_w_info!(vga_text::VGA_TEXT).backspace();
                }
                Key::Tab => self.input_buffer.push('\t'),
                Key::Q => self.input_buffer.push('q'),
                Key::W => self.input_buffer.push('w'),
                Key::E => self.input_buffer.push('e'),
                Key::R => self.input_buffer.push('r'),
                Key::T => self.input_buffer.push('t'),
                Key::Y => self.input_buffer.push('y'),
                Key::U => self.input_buffer.push('u'),
                Key::I => self.input_buffer.push('i'),
                Key::O => self.input_buffer.push('o'),
                Key::P => self.input_buffer.push('p'),
                Key::LeftBracket => self.input_buffer.push('['),
                Key::RightBracket => self.input_buffer.push(']'),
                Key::Enter | Key::NumEnter => {
                    self.send_line(false);
                }
                Key::A => self.input_buffer.push('a'),
                Key::S => self.input_buffer.push('s'),
                Key::D => self.input_buffer.push('d'),
                Key::F => self.input_buffer.push('f'),
                Key::G => self.input_buffer.push('g'),
                Key::H => self.input_buffer.push('h'),
                Key::J => self.input_buffer.push('j'),
                Key::K => self.input_buffer.push('k'),
                Key::L => self.input_buffer.push('l'),
                Key::Semicolon => self.input_buffer.push(';'),
                Key::Quote => self.input_buffer.push('\''),
                Key::Backslash => self.input_buffer.push('\\'),
                Key::Z => self.input_buffer.push('z'),
                Key::X => self.input_buffer.push('x'),
                Key::C => self.input_buffer.push('c'),
                Key::V => self.input_buffer.push('v'),
                Key::B => self.input_buffer.push('b'),
                Key::N => self.input_buffer.push('n'),
                Key::M => self.input_buffer.push('m'),
                Key::Comma => self.input_buffer.push(','),
                Key::Dot => self.input_buffer.push('.'),
                Key::Slash => self.input_buffer.push('/'),
                Key::Space => self.input_buffer.push(' '),
                Key::NumDot => self.input_buffer.push('.'),
                Key::NumSlash => self.input_buffer.push('/'),
                Key::NumAsterisk => self.input_buffer.push('*'),
                Key::NumMinus => self.input_buffer.push('-'),
                Key::NumPlus => self.input_buffer.push('+'),
                _ => {}
            }
        } else {
            match input {
                Key::Backtick => self.input_buffer.push('~'),
                Key::K1 => self.input_buffer.push('!'),
                Key::K2 => self.input_buffer.push('@'),
                Key::K3 => self.input_buffer.push('#'),
                Key::K4 => self.input_buffer.push('$'),
                Key::K5 => self.input_buffer.push('%'),
                Key::K6 => self.input_buffer.push('^'),
                Key::K7 => self.input_buffer.push('&'),
                Key::K8 => self.input_buffer.push('*'),
                Key::K9 => self.input_buffer.push('('),
                Key::K0 => self.input_buffer.push(')'),
                Key::Dash => self.input_buffer.push('_'),
                Key::Equal => self.input_buffer.push('+'),
                Key::Backspace => {
                    self.input_buffer.pop();
                    lock_w_info!(vga_text::VGA_TEXT).backspace();
                }
                Key::Tab => self.input_buffer.push('\t'),
                Key::Q => self.input_buffer.push('Q'),
                Key::W => self.input_buffer.push('W'),
                Key::E => self.input_buffer.push('E'),
                Key::R => self.input_buffer.push('R'),
                Key::T => self.input_buffer.push('T'),
                Key::Y => self.input_buffer.push('Y'),
                Key::U => self.input_buffer.push('U'),
                Key::I => self.input_buffer.push('I'),
                Key::O => self.input_buffer.push('O'),
                Key::P => self.input_buffer.push('P'),
                Key::LeftBracket => self.input_buffer.push('{'),
                Key::RightBracket => self.input_buffer.push('}'),
                Key::Enter | Key::NumEnter => {
                    self.send_line(false);
                }
                Key::A => self.input_buffer.push('A'),
                Key::S => self.input_buffer.push('S'),
                Key::D => self.input_buffer.push('D'),
                Key::F => self.input_buffer.push('F'),
                Key::G => self.input_buffer.push('G'),
                Key::H => self.input_buffer.push('H'),
                Key::J => self.input_buffer.push('J'),
                Key::K => self.input_buffer.push('K'),
                Key::L => self.input_buffer.push('L'),
                Key::Semicolon => self.input_buffer.push(':'),
                Key::Quote => self.input_buffer.push('"'),
                Key::Backslash => self.input_buffer.push('|'),
                Key::Z => self.input_buffer.push('Z'),
                Key::X => self.input_buffer.push('X'),
                Key::C => self.input_buffer.push('C'),
                Key::V => self.input_buffer.push('V'),
                Key::B => self.input_buffer.push('B'),
                Key::N => self.input_buffer.push('N'),
                Key::M => self.input_buffer.push('M'),
                Key::Comma => self.input_buffer.push('<'),
                Key::Dot => self.input_buffer.push('>'),
                Key::Slash => self.input_buffer.push('?'),
                Key::Space => self.input_buffer.push(' '),
                Key::NumDot => self.input_buffer.push('.'),
                Key::NumSlash => self.input_buffer.push('/'),
                Key::NumAsterisk => self.input_buffer.push('*'),
                Key::NumMinus => self.input_buffer.push('-'),
                Key::NumPlus => self.input_buffer.push('+'),
                Key::Num1 => self.input_buffer.push('1'),
                Key::Num2 => self.input_buffer.push('2'),
                Key::Num3 => self.input_buffer.push('3'),
                Key::Num4 => self.input_buffer.push('4'),
                Key::Num5 => self.input_buffer.push('5'),
                Key::Num6 => self.input_buffer.push('6'),
                Key::Num7 => self.input_buffer.push('7'),
                Key::Num8 => self.input_buffer.push('8'),
                Key::Num9 => self.input_buffer.push('9'),
                Key::Num0 => self.input_buffer.push('0'),
                _ => {}
            }
        }

        let buf_to_fill_size_after = self.input_buffer.len();
        if buf_to_fill_size_after > buf_to_fill_size_before {
            let last_char_bytes = &self.input_buffer.as_bytes()[buf_to_fill_size_before..];
            let last_char = unsafe { core::str::from_utf8_unchecked(last_char_bytes) };
            self.print(last_char);
        }

        if self.running_pid.is_none() && !self.started_proc && !self.done_streams.is_empty() {
            self.start_proc();
        }
    }

    pub fn print(&self, data: &str) {
        lock_w_info!(vga_text::VGA_TEXT).write_text(data);
    }

    fn send_line(&mut self, eof_line: bool) {
        let mut done_stream = core::mem::take(&mut self.input_buffer);
        if !eof_line {
            done_stream.push('\n');
        }
        self.done_streams.push((done_stream, eof_line));
        lock_w_info!(vga_text::VGA_TEXT).do_newline();
    }

    fn start_proc(&mut self) {
        let mut launch_command = self.done_streams.pop().unwrap_or((String::new(), false)).0;
        let last_char = launch_command.pop(); //remove newline
        if last_char != Some('\n') {
            //if it was not a newline, put it back
            if let Some(last_char) = last_char {
                launch_command.push(last_char);
            }
        }

        let mut chunks = proc::CommandSplitter::new(&launch_command);
        let Some(program_path) = chunks.next() else {
            //empty input
            return;
        };
        if !program_path.starts_with("/") {
            self.print("only absolute paths are allowed in a tty\n");
            return;
        }

        let resolved_path = vfs::resolve_path(&program_path);

        let start_tty_proc_future = async move {
            let run_proc_future = proc::run_process_default_env((&resolved_path).into(), &launch_command);
            match run_proc_future.await {
                Ok(pid) => {
                    lock_w_info!(TTY).running_pid = Some(pid);
                }
                Err(e) => {
                    lock_w_info!(TTY).print(&format!("Failed to start process: {}, error: {:?}\n", program_path, e));
                }
            }
        };
        crate::task_runner::add_task(Box::pin(start_tty_proc_future), None);
    }
}
