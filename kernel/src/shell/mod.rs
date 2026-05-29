use crate::{
    memory,
    proc::{self, Pid},
    shell::{cmd_cat::cmd_cat, cmd_ls::cmd_ls},
    task_runner::PidOption,
    tty::TTY,
    vfs::{self, ResolvedPath, ResolvedPathBorrowed},
};
use std::{
    format, lock_w_info, println,
    string::{String, ToString},
    sync::no_int_spinlock::NoIntSpinlock,
};

mod cmd_cat;
mod cmd_ls;

pub struct ShellState {
    current_dir: Option<ResolvedPath>,
    running_proc: Option<Pid>,
    started_proc: bool,
}

pub static SHELL_STATE: NoIntSpinlock<ShellState> = NoIntSpinlock::new(ShellState {
    current_dir: None,
    running_proc: None,
    started_proc: false,
});

pub fn init() {
    let mut shell_state = lock_w_info!(SHELL_STATE);
    shell_state.current_dir = Some(ResolvedPath::root());
    shell_state.print_prompt();
}

impl ShellState {
    pub fn consume_shell_command(&mut self, cmd: (String, bool)) {
        if self.current_dir.is_none() {
            self.current_dir = Some(ResolvedPath::root());
        }

        let (cmd, _) = cmd;

        let cmd = cmd.trim();

        let mut chunks = proc::CommandSplitter::new(cmd);
        let Some(cmd_name) = chunks.next() else {
            self.command_finished();
            return;
        };

        match cmd_name.as_str() {
            "ls" => {
                println!("ls command executed");
                let path = chunks.next().unwrap_or_else(|| ".".to_string());
                self.started_proc = true;
                let task = async move {
                    let res = cmd_ls(&path).await;
                    match res {
                        Ok(()) => lock_w_info!(SHELL_STATE).command_finished(),
                        Err(e) => {
                            lock_w_info!(TTY).print(&format!("Error executing ls command: {:?}\n", e));
                            lock_w_info!(SHELL_STATE).command_finished();
                        }
                    }
                };
                let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);
                crate::task_runner::add_task(ffi_safe_task, PidOption::None);
            }
            "cat" => {
                println!("cat command executed");
                let path = chunks.next().unwrap_or_else(|| ".".to_string());
                self.started_proc = true;
                let task = async move {
                    let res = cmd_cat(&path).await;
                    match res {
                        Ok(()) => lock_w_info!(SHELL_STATE).command_finished(),
                        Err(e) => {
                            lock_w_info!(TTY).print(&format!("Error executing cat command: {:?}\n", e));
                            lock_w_info!(SHELL_STATE).command_finished();
                        }
                    }
                };
                let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);
                crate::task_runner::add_task(ffi_safe_task, PidOption::None);
            }
            "mmap" => {
                println!("mmap command executed");
                memory::print_mem_mapping();
                self.command_finished();
            }
            prog_path if prog_path.starts_with("/") => {
                println!("Executing file operation command: {}", prog_path);
                let resolved_path = vfs::resolve_path(prog_path);

                let cmd_cloned = cmd.to_string();
                let prog_path_cloned = prog_path.to_string();

                self.started_proc = true;

                let task = async move {
                    let run_proc_future = proc::run_process_default_env((&resolved_path).into(), &cmd_cloned, "/").await;
                    match run_proc_future {
                        Ok(pid) => {
                            let mut self_state = lock_w_info!(SHELL_STATE);
                            self_state.running_proc = Some(pid);
                            self_state.started_proc = false;
                        }
                        Err(e) => {
                            lock_w_info!(TTY).print(&format!("Failed to start process: {}, error: {:?}\n", prog_path_cloned, e));
                            lock_w_info!(SHELL_STATE).command_finished();
                        }
                    }
                };
                let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);
                crate::task_runner::add_task(ffi_safe_task, PidOption::None);
            }
            _ => {
                lock_w_info!(TTY).print(&format!("Unknown command: {}\n", cmd_name));
                self.command_finished();
            }
        }
    }

    pub fn can_consume_shell_command(&self) -> bool {
        self.running_proc.is_none() && !self.started_proc
    }

    fn command_finished(&mut self) {
        self.running_proc = None;
        self.started_proc = false;
        self.print_prompt();
    }

    fn print_prompt(&self) {
        let path: String = self
            .current_dir
            .as_ref()
            .map(|p| {
                let borrowed = ResolvedPathBorrowed::from(p);
                borrowed.to_string()
            })
            .unwrap_or_else(|| "/".to_string());
        lock_w_info!(TTY).print(&format!("\n{}> ", path));
    }

    pub fn kill_proc(&mut self) {
        if let Some(pid) = self.running_proc {
            proc::kill_process(pid, 0);
            self.running_proc = None;
            self.started_proc = false;
        }

        self.print_prompt();
    }
}
