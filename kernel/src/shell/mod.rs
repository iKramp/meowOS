use crate::{
    proc::{self, Pid},
    shell::{
        cmd_cat::cmd_cat, cmd_cp::cmd_cp, cmd_ls::cmd_ls, cmd_mkdir::cmd_mkdir, cmd_mmap::cmd_mmap, cmd_mount::cmd_mount,
        cmd_tree::cmd_tree,
    },
    task_runner::{PidOption, add_repeating_task, yield_now},
    tty::TTY,
    vfs::{self, ResolvedPath, ResolvedPathBorrowed},
};
use core::pin::Pin;
use std::{
    alloc::borrow::ToOwned,
    boxed::Box,
    error::KernelError,
    format, lock_w_info, println,
    string::{String, ToString},
    sync::no_int_spinlock::NoIntSpinlock,
    vec::Vec,
};

mod cmd_cat;
mod cmd_cp;
mod cmd_ls;
mod cmd_mkdir;
mod cmd_mmap;
mod cmd_mount;
mod cmd_rm;
mod cmd_tree;

type AsyncCommandRetType = Pin<Box<dyn std::future::Future<Output = Result<(), KernelError>> + Send>>;
type AsyncCmd = fn(proc::CommandSplitter) -> AsyncCommandRetType;
type SyncCmd = fn(proc::CommandSplitter) -> Result<(), KernelError>;

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

static ASYNC_CMDS: &[(&str, AsyncCmd)] = &[
    ("ls", cmd_ls),
    ("cat", cmd_cat),
    ("mount", cmd_mount),
    ("mkdir", cmd_mkdir),
    ("cp", cmd_cp),
    ("tree", cmd_tree),
];
static SYNC_CMDS: &[(&str, SyncCmd)] = &[("mmap", cmd_mmap)];

fn update_shell() {
    let mut shell_state = lock_w_info!(SHELL_STATE);
    if let Some(pid) = shell_state.running_proc {
        let proc = proc::get_proc(pid);
        if proc.is_none() {
            shell_state.command_finished();
        }
    }
}

pub fn init(init_commands: Vec<String>) {
    let mut shell_state = lock_w_info!(SHELL_STATE);
    shell_state.current_dir = Some(ResolvedPath::root());
    shell_state.print_prompt();
    add_repeating_task(Box::new(update_shell));
    drop(shell_state);

    let task = Box::pin(async move {
        for cmd in init_commands {
            let mut shell_state = lock_w_info!(SHELL_STATE);
            while !shell_state.can_consume_shell_command() {
                drop(shell_state);
                yield_now().await;
                shell_state = lock_w_info!(SHELL_STATE);
            }
            shell_state.consume_shell_command((cmd, false));
        }
    });

    let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);
    crate::task_runner::add_task(ffi_safe_task, PidOption::None);
}

impl ShellState {
    pub fn consume_shell_command(&mut self, cmd: (String, bool)) {
        if self.current_dir.is_none() {
            self.current_dir = Some(ResolvedPath::root());
        }

        println!("Received command: {}", cmd.0);

        let (cmd, _) = cmd;

        let cmd = cmd.trim();

        let mut chunks = proc::CommandSplitter::new(cmd);
        let Some(cmd_name) = chunks.next() else {
            self.command_finished();
            return;
        };

        let args_clone = chunks.clone();
        println!("Command: {}, Args: {:?}", cmd_name, args_clone);

        let sync_cmd = SYNC_CMDS.iter().find(|&&cmd| cmd_name == cmd.0);
        if let Some((_, cmd)) = sync_cmd {
            println!("Executing sync command: {}", cmd_name);
            self.run_sync_cmd(*cmd, chunks);
            return;
        }

        let async_cmd = ASYNC_CMDS.iter().find(|&&cmd| cmd_name == cmd.0);
        if let Some((_, cmd)) = async_cmd {
            println!("Executing async command: {}", cmd_name);
            self.run_async_cmd(*cmd, chunks);
            return;
        }

        let success = self.try_execute_program(&cmd_name, chunks);
        if success {
            println!("Executing program: {}", cmd_name);
            return;
        }

        lock_w_info!(TTY).print(&format!("Unknown command: {}\n", cmd_name));
        self.command_finished();
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

    fn run_async_cmd(&mut self, cmd: AsyncCmd, args: proc::CommandSplitter) {
        self.started_proc = true;
        let fut = async move {
            let args_clone = args.clone();
            let res = cmd(args).await;
            if let Err(e) = res {
                lock_w_info!(TTY).print(&format!("Error executing command: {:?} with args: {:?}\n", e, args_clone));
            }
            lock_w_info!(crate::shell::SHELL_STATE).command_finished();
        };

        let ffi_safe_task = std::ffi_future::future::into_ffi_future(fut);
        crate::task_runner::add_task(ffi_safe_task, PidOption::None);
    }

    fn run_sync_cmd(&mut self, cmd: SyncCmd, args: proc::CommandSplitter) {
        let res = cmd(args);
        if let Err(e) = res {
            lock_w_info!(TTY).print(&format!("Error executing command: {:?}\n", e));
        }
        self.command_finished();
    }

    fn try_execute_program(&mut self, prog_path: &str, args: proc::CommandSplitter) -> bool {
        if !prog_path.starts_with("/") && !prog_path.starts_with("./") && !prog_path.starts_with("../") {
            return false;
        }

        println!("Executing file operation command: {}", prog_path);
        let resolved_path = vfs::resolve_path(prog_path);

        let mut cmd_cloned = prog_path.to_owned();
        for arg in args {
            cmd_cloned.push(' ');
            cmd_cloned.push_str(&arg);
        }
        let prog_path = prog_path.to_owned();

        self.started_proc = true;

        let task = Box::pin(async move {
            let run_proc_future = proc::run_process_default_env((&resolved_path).into(), &cmd_cloned, "/").await;
            match run_proc_future {
                Ok(pid) => {
                    let mut self_state = lock_w_info!(SHELL_STATE);
                    self_state.running_proc = Some(pid);
                    self_state.started_proc = false;
                }
                Err(e) => {
                    lock_w_info!(TTY).print(&format!("Failed to start process: {}, error: {:?}\n", prog_path, e));
                    lock_w_info!(SHELL_STATE).command_finished();
                }
            }
        });
        let ffi_safe_task = std::ffi_future::future::into_ffi_future(task);
        crate::task_runner::add_task(ffi_safe_task, PidOption::None);
        true
    }
}
