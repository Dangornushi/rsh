use crate::command;
use crate::error::error::{RshError, Status};
use crate::log::log_maneger::csv_writer;
use crate::parser::parse::{Command, CompoundStatement, Identifier, Node};
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
use std::io::stdout;
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
                    waitpid(child, None).map_err(|err| RshError::new(&format!("{}", err)));

                tcsetpgrp(0, getpgrp()).unwrap();

                match wait_pid_result {
                    Ok(WaitStatus::Exited(_, return_code)) => {
                        // ui
                        //self.return_code = return_code;
                        Ok(Status::Success)
                    }
                    Ok(WaitStatus::Signaled(_, _, _)) => {
                        println!("signaled");
                        Ok(Status::Success)
                    }
                    Err(err) => {
                        //self.eprintln(&format!("rsh: {}", err.message));
                        Ok(Status::Success)
                    }
                    _ => Ok(Status::Success),
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

                execvp(&path, &c_args)
                    .map_err(|_| RshError::new(&format!("{} is not found", args[0])))?;
                Ok(Status::Success)

                // -------------
            }
        }
    }

    pub fn rsh_execute(&mut self, args: Vec<String>) -> Result<Status, RshError> {
        if let Option::Some(arg) = args.get(0) {
            let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let path = self.rsh.open_profile(".rsh_history")?;

            let _ = csv_writer(args.join(" "), time, &path);

            if let Ok(r) = match arg.as_str() {
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
                        Ok(Status::Success)
                    }
                    _ => Ok(Status::Success),
                },
                // ロゴ表示
                "%logo" => command::logo::rsh_logo(),
                // history: 履歴表示の組み込みコマンド
                "%fl" => command::history::rsh_history(self.rsh.get_history_database())
                    .map(|_| Status::Success),
                // exit: 終了用の組み込みコマンド
                "exit" => command::exit::rsh_exit(),
                // none: 何もなければコマンド実行
                _ => self.rsh_launch(args),
            } {
                return Ok(r);
            } else {
                //self.eprintln("Failed to execute command");
                return Err(RshError::new("Failed to execute command"));
            }
        } else {
            return Ok(Status::Success);
        }
    }

    fn eval_identifier(&self, expr: Identifier) -> String {
        expr.eval()
    }

    fn eval_command(&mut self, expr: Command) {
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
            Ok(Status::Exit) => {
                std::process::exit(0);
            }
            Ok(_) => {
                self.rsh.rsh_print("fin".to_string());
            }
            Err(_) => {
                self.rsh.rsh_print("Error: command not found".to_string());
            }
        }
    }

    fn eval_compound_statement(&mut self, expr: CompoundStatement) {
        let expr = expr.eval();
        for s in expr {
            match s {
                Node::Command(command) => {
                    self.eval_command(*command);
                }
                _ => {}
            }
        }
    }

    pub fn evaluate(&mut self, ast: Node) {
        // ASTを評価
        match ast {
            Node::CompoundStatement(stmt) => {
                self.eval_compound_statement(stmt);
            }
            _ => {}
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_compound_statement() {
        let rsh = Rsh::new(); // Adjust with appropriate initialization
        let mut evaluator = Evaluator::new(rsh);
        let compound_statement = CompoundStatement::new(vec![]); // Adjust with appropriate initialization
        evaluator.eval_compound_statement(compound_statement);
        // Add assertions here to verify the expected behavior
    }

    #[test]
    fn test_evaluate_with_compound_statement() {
        let rsh = Rsh::new(); // Adjust with appropriate initialization
        let mut evaluator = Evaluator::new(rsh);
        let compound_statement = CompoundStatement::new(vec![]); // Adjust with appropriate initialization
        let ast = Node::CompoundStatement(compound_statement);
        evaluator.evaluate(ast);
    }

    #[test]
    fn test_evaluate_with_other_node() {
        let rsh = Rsh::new(); // Adjust with appropriate initialization
        let mut evaluator = Evaluator::new(rsh);
        let other_node = Node::Identifier(Identifier::new("hello".to_string())); // Replace with an actual variant of Node
        let result = evaluator.evaluate(other_node);
        evaluator.evaluate(other_node);
    }
    #[test]
    fn test_eval_command_with_identifier() {
        let rsh = Rsh::new(); // Adjust with appropriate initialization
        let mut evaluator = Evaluator::new(rsh);
        let identifier = Identifier::new("test_command".to_string());
        let command = Command::new(Node::Identifier(identifier.clone()), vec![]);
        evaluator.eval_command(command);
        // Add assertions here to verify the expected behavior
    }

    #[test]
    fn test_eval_command_with_sub_commands() {
        let rsh = Rsh::new(); // Adjust with appropriate initialization
        let mut evaluator = Evaluator::new(rsh);
        let identifier = Identifier::new("echo".to_string());
        let sub_identifier = Identifier::new("hello, world".to_string());
        let command = Command::new(
            Node::Identifier(identifier.clone()),
            vec![Node::Identifier(sub_identifier.clone())],
        );
        evaluator.eval_command(command);
        // Add assertions here to verify the expected behavior
    }

    #[test]
    fn test_eval_command_with_non_identifier() {
        let rsh = Rsh::new(); // Adjust with appropriate initialization
        let mut evaluator = Evaluator::new(rsh);
        let non_identifier_node = Node::CompoundStatement(CompoundStatement::new(vec![])); // Replace with an actual non-identifier variant of Node
        let command = Command::new(non_identifier_node, vec![]);
        evaluator.eval_command(command);
        // 期待される動作を確認するためのアサーションを追加
    }
}
