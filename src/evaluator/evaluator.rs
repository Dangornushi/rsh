use crate::command;
use crate::error::error::{RshError, Status, StatusCode};
use crate::log::log_maneger::csv_writer;
use crate::parser::parse::{CommandStatement, CompoundStatement, Define, Identifier, Node};
use crate::rsh::rsh::Rsh;
use crossterm::{execute, style::Print};
use nix::{
    errno::Errno,
    libc,
    sys::{
        signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal},
        wait::*,
    },
    unistd::{close, execvp, fork, getpgrp, pipe, setpgid, tcsetpgrp, ForkResult},
};
use std::any::Any;
use std::io::stdout;
use std::process::Command;
use std::{ffi::CString, io::Write};

pub struct Evaluator {
    rsh: Rsh,
}

impl Evaluator {
    pub fn new(rsh: Rsh) -> Self {
        Evaluator { rsh }
    }

    fn restore_tty_signals(&self) {
        let sa = SigAction::new(SigHandler::SigDfl, SaFlags::empty(), SigSet::empty());
        unsafe {
            sigaction(Signal::SIGTSTP, &sa).unwrap();
            sigaction(Signal::SIGTTIN, &sa).unwrap();
            sigaction(Signal::SIGTTOU, &sa).unwrap();
        }
    }

    fn rsh_launch(&mut self, args: Vec<String>) -> Result<Status, RshError> {
        let pid = fork().map_err(|_| RshError::new("fork failed"))?;
        let (pipe_read, pipe_write) = pipe().unwrap();

        match pid {
            ForkResult::Parent { child } => {
                setpgid(child, child).unwrap();
                tcsetpgrp(0, child).unwrap();
                close(pipe_read).unwrap();
                close(pipe_write).unwrap();

                let wait_pid_result =
                    waitpid(child, None).map_err(|err| RshError::new(&format!("waited: {}", err)));

                tcsetpgrp(0, getpgrp()).unwrap();

                match wait_pid_result {
                    Ok(WaitStatus::Exited(_, return_code)) => {
                        // ui
                        match return_code {
                            0 => Ok(Status::success()),
                            1 => Err(RshError::new("not found")),
                            _ => Err(RshError::new("somthing wrong")),
                        }
                    }
                    Ok(WaitStatus::Signaled(_, _, _)) => Ok(Status::success()),
                    Err(err) => {
                        println!("parent err: {}", err.message);
                        //self.eprintln(&format!("rsh: {}", err.message));
                        Err(err)
                    }
                    _ => Ok(Status::success()),
                }
            }
            ForkResult::Child => {
                // シグナル系処理 ---------------------------
                self.restore_tty_signals();

                close(pipe_write).unwrap();

                loop {
                    let mut buf = [0];
                    match nix::unistd::read(pipe_read, &mut buf) {
                        Err(e) if e == nix::Error::Sys(Errno::EINTR) => (),
                        _ => break,
                    }
                    unsafe {
                        if libc::isatty(libc::STDIN_FILENO) == 1 {
                            let mut sigset = SigSet::empty();
                            sigset.add(Signal::SIGINT);
                            sigset.add(Signal::SIGQUIT);
                            sigset.add(Signal::SIGTERM);
                            if sigset.contains(Signal::SIGINT) {
                                libc::_exit(0);
                            }
                        }
                    }
                }
                close(pipe_read).unwrap();
                // ------------------------------------------

                // コマンドパース
                let path = CString::new(args[0].to_string()).unwrap();

                let c_args: Vec<CString> = args
                    .iter()
                    .map(|s| CString::new(s.as_bytes()).unwrap())
                    .collect();

                match execvp(&path, &c_args) {
                    Ok(_) => Ok(Status::success()),
                    Err(err) => Err(RshError::new(format!("{:?}", err).as_str())),
                }

                // -------------
            }
        }
    }

    pub fn rsh_execute(&mut self, args: Vec<String>) -> Result<Status, RshError> {
        if let Option::Some(arg) = args.get(0) {
            let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let path = self.rsh.open_profile(".rsh_history")?;

            let _ = csv_writer(args.join(" "), time, &path);

            match arg.as_str() {
                r => match r {
                    // cd: ディレクトリ移動の組み込みコマンド
                    "cd" => match command::cd::rsh_cd(if let Option::Some(dir) = args.get(1) {
                        dir
                    } else {
                        execute!(stdout(), Print("\n")).unwrap();
                        std::io::stdout().flush().unwrap();
                        "./"
                    }) {
                        Err(err) => {
                            self.rsh.eprintln(&format!("Error: {}", err.message));
                            Ok(Status::success())
                        }
                        _ => Ok(Status::success()),
                    },
                    // ロゴ表示
                    "%logo" => command::logo::rsh_logo(),
                    // history: 履歴表示の組み込みコマンド
                    "%fl" => command::history::rsh_history(self.rsh.get_history_database())
                        .map(|_| Status::success()),
                    // exit: 終了用の組み込みコマンド
                    "exit" => command::exit::rsh_exit(),
                    // none: 何もなければコマンド実行
                    _ => {
                        match Command::new(args[0].clone()).args(&args[1..]).spawn() {
                            Ok(mut child) => child
                                .wait()
                                .map(|status| {
                                    if status.success() {
                                        Status::success()
                                    } else {
                                        Status::notfound()
                                    }
                                })
                                .map_err(|err| RshError::new(&format!("Error: {}", err))),
                            Err(err) => Err(RshError::new(&format!("Error: {}", err))),
                        }
                        //self.rsh_launch(args),
                    }
                },
            }
        } else {
            return Err(RshError::new("Failed to get args"));
        }
    }

    fn eval_identifier(&self, expr: Identifier) -> String {
        expr.eval()
    }

    fn eval_command(&mut self, expr: CommandStatement) {
        let command = match expr.get_command() {
            Node::Identifier(identifier) => self.eval_identifier(identifier.clone()),
            _ => {
                // Provide a default value or handle the case where the command is not an identifier
                Default::default() // Replace with an appropriate default value
            }
        };
        let sub_command = expr
            .get_sub_command()
            .into_iter()
            .map(|node| match node {
                Node::Identifier(identifier) => identifier.eval(),
                _ => Default::default(), // Handle other cases appropriately
            })
            .collect::<Vec<String>>();

        let mut full_command = vec![command];
        full_command.extend(sub_command);

        // 分割したコマンドを実行
        match self.rsh_execute(full_command.clone()) {
            Ok(r) => {
                if r.get_status_code() == StatusCode::Exit {
                    std::process::exit(0);
                }
                let return_code = r.get_exit_code();
                //println!("Evaluator Exit: {}", return_code);
            }
            Err(err) => {
                println!("Evaluator-{}", err.message);
                //std::process::exit(1);
            }
        }
    }

    fn eval_define(&mut self, define: Define) {
        let var = match define.get_var() {
            Node::Identifier(identifier) => self.eval_identifier(identifier),
            _ => Default::default(), // Handle other cases appropriately
        };
        let data = match define.get_data() {
            Node::Identifier(identifier) => self.eval_identifier(identifier),
            _ => Default::default(), // Handle other cases appropriately
        };
        println!("{}: {}\n", var, data);
    }

    fn eval_compound_statement(&mut self, expr: CompoundStatement) {
        let expr = expr.eval();
        for s in expr {
            match s {
                Node::CommandStatement(command) => {
                    self.eval_command(*command);
                }
                Node::Define(define) => {
                    self.eval_define(*define);
                }
                /*
                 */
                _ => {
                    println!("error: {:?}", s);
                }
            }
        }
    }

    pub fn evaluate(&mut self, ast: Node) -> i32 {
        // ASTを評価
        match ast {
            Node::CompoundStatement(stmt) => {
                self.eval_compound_statement(stmt);
                0
            }
            Node::Identifier(identifier) => {
                self.eval_identifier(identifier);
                0
            }
            _ => 1,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_restore_tty_signals() {
        let rsh = Rsh::new();
        let evaluator = Evaluator::new(rsh);
        evaluator.restore_tty_signals();
        // Add assertions or checks if possible
    }
    /*

    #[test]
    fn test_rsh_launch_success() {
        let rsh = Rsh::new();
        let mut evaluator = Evaluator::new(rsh);
        let args = vec!["echo".to_string(), "Hello, world!".to_string()];
        let result = evaluator.rsh_launch(args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Status::Success);
    }
    #[test]
    fn test_rsh_launch_failure() {
        let rsh = Rsh::new();
        let mut evaluator = Evaluator::new(rsh);
        let args = vec!["nonexistent_command".to_string()];
        let result = evaluator.rsh_launch(args);
        assert!(result.is_err());
    }
    */

    #[test]
    fn test_rsh_execute_cd() {
        let rsh = Rsh::new();
        let mut evaluator = Evaluator::new(rsh);
        let args = vec!["cd".to_string(), "/".to_string()];
        let result = evaluator.rsh_execute(args);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Status::success());
    }

    #[test]
    fn test_eval_identifier() {
        let rsh = Rsh::new();
        let evaluator = Evaluator::new(rsh);
        let identifier = Identifier::new("test".to_string());
        let result = evaluator.eval_identifier(identifier);
        assert_eq!(result, "test");
    }

    /*
    #[test]
    fn test_eval_command() {
        let rsh = Rsh::new();
        let mut evaluator = Evaluator::new(rsh);
        let command = Command::new(
            Node::Identifier(Identifier::new("echo".to_string())),
            vec![Node::Identifier(Identifier::new("Hello".to_string()))],
        );
        evaluator.eval_command(command);
        // Add assertions or checks if possible
    }

    #[test]
    fn test_eval_compound_statement() {
        let rsh = Rsh::new();
        let mut evaluator = Evaluator::new(rsh);
        let command = Command::new(
            Node::Identifier(Identifier::new("echo".to_string())),
            vec![Node::Identifier(Identifier::new("Hello".to_string()))],
        );
        let compound_statement = CompoundStatement::new(vec![Node::Command(Box::new(command))]);
        evaluator.eval_compound_statement(compound_statement);
        // Add assertions or checks if possible
    }

    #[test]
    fn test_evaluate() {
        let rsh = Rsh::new();
        let mut evaluator = Evaluator::new(rsh);
        let command = Command::new(
            Node::Identifier(Identifier::new("echo".to_string())),
            vec![Node::Identifier(Identifier::new("Hello".to_string()))],
        );
        let ast = Node::CompoundStatement(CompoundStatement::new(vec![Node::Command(Box::new(
            command,
        ))]));
        evaluator.evaluate(ast);
        // Add assertions or checks if possible
    }
     */
}
