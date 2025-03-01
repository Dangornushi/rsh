use crate::command;
use crate::error::error::{RshError, Status, StatusCode};
use crate::log::log_maneger::csv_writer;
use crate::parser::parse::{
    CommandStatement, CompoundStatement, Define, ExecScript, Identifier, Node, Pipeline,
};
use crate::rsh::rsh::Rsh;
use crossterm::{execute, style::Print};
use nix::unistd::dup2;
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
use std::io::{stdout, Read};
use std::process::Command;
use std::{ffi::CString, io::Write};

enum Process {
    Pipe,
    NoPipe,
}

pub struct Evaluator {
    rsh: Rsh,
    now_process: Process,
    pipe_commands: Vec<Vec<String>>,
}

impl Evaluator {
    pub fn new(rsh: Rsh) -> Self {
        Evaluator {
            rsh,
            now_process: Process::NoPipe,
            pipe_commands: Vec::new(),
        }
    }

    fn switch_process(&mut self, process: Process) {
        self.now_process = process;
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
        let (pipe_read, pipe_write) = pipe().unwrap();
        let pid = fork().map_err(|_| RshError::new("fork failed"))?;
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

    fn rsh_pipe_launch(&mut self, args: Vec<Vec<String>>) -> Result<Status, RshError> {
        let mut pipes = Vec::new();

        for _ in 0..args.len() - 1 {
            let (pipe_read, pipe_write) = pipe().unwrap();
            pipes.push((pipe_read, pipe_write));
        }

        for (i, command) in args.iter().enumerate() {
            let pid = fork().map_err(|_| RshError::new("fork failed"))?;
            match pid {
                ForkResult::Parent { child } => {
                    setpgid(child, child).unwrap();
                    tcsetpgrp(0, child).unwrap();

                    if i > 0 {
                        close(pipes[i - 1].0).unwrap();
                    }
                    if i < args.len() - 1 {
                        close(pipes[i].1).unwrap();
                    }

                    let wait_pid_result = waitpid(child, None)
                        .map_err(|err| RshError::new(&format!("waited: {}", err)));

                    tcsetpgrp(0, getpgrp()).unwrap();
                    println!("wait_pid_result: {:?}", wait_pid_result);

                    return match wait_pid_result {
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
                    };
                }
                ForkResult::Child => {
                    // シグナル系処理 ---------------------------
                    self.restore_tty_signals();

                    if i > 0 {
                        close(pipes[i - 1].1).unwrap();
                        dup2(pipes[i - 1].0, 0).unwrap();
                        close(pipes[i - 1].0).unwrap();
                    }
                    if i < args.len() - 1 {
                    println!("start child: {}, {:?}", i, args);
                        close(pipes[i].0).unwrap();
                        dup2(pipes[i].1, 1).unwrap();
                        close(pipes[i].1).unwrap();
                    }
                    // ------------------------------------------

                    println!("command: {:?}", command);
                    // コマンドパース
                    let path = CString::new(command[0].clone()).unwrap();

                    let c_args: Vec<CString> = command
                        .iter()
                        .map(|s| CString::new(s.as_bytes()).unwrap())
                        .collect();

                    println!("path: {:?}", path);

                    return match execvp(&path, &c_args) {
                        Ok(_) => Ok(Status::success()),
                        Err(err) => Err(RshError::new(format!("{:?}", err).as_str())),
                    };
                    // -------------
                }
            }
        }

        for _ in 0..args.len() {
            wait().unwrap();
        }
        Ok(Status::success())
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
                        #[cfg(test)]{
                            // テスト時には何もしない
                            println!("test");
                            Ok(Status::success())
                        }
                        #[cfg(not(test))]
                        {
                            match self.now_process {
                                Process::NoPipe => self.rsh_launch(args),
                                Process::Pipe => {
                                    self.pipe_commands.push(args);
                                    Ok(Status::success())
                                }
                            }
                        }
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
        println!("{} = {}\n", var, data);
    }

    fn eval_pipeline(&mut self, pipeline: Pipeline) {
        self.switch_process(Process::Pipe);
        self.pipe_commands.clear();
        // パイプライン処理
        for command in pipeline.get_commands() {
            match command.get_node() {
                Node::CommandStatement(command) => self.eval_command(*command),
                _ => println!("I don't know: {:?}", command),
            }
        }
        match self.rsh_pipe_launch(self.pipe_commands.clone()) {
            Ok(s) => println!("Pipeline execution success: {:?}", s),
            Err(err) => println!("Pipeline execution error: {}", err.message),
        }
        self.pipe_commands.clear();
        self.switch_process(Process::NoPipe);

        /*
        return ;*/
    }

    fn eval_branch(&mut self, node: Node) -> impl Any {
        // 変数などのデータ型を戻り値として返すようにする？
        // 変数格納のハッシュマップ
        // 関数格納のハッシュマップ
        // 今いる関数
        // exit code
    }

    fn eval_exec_script(&mut self, script: ExecScript) {
        // スクリプトを実行
        let var = match script.get_filename() {
            Node::Identifier(identifier) => self.eval_identifier(identifier),
            _ => Default::default(), // Handle other cases appropriately
        };
        if let Ok(mut file) = std::fs::File::open(var.clone()) {
            let mut contents = String::new();
            if let Ok(_) = file.read_to_string(&mut contents) {
                let return_code = Rsh::execute_commands(&mut self.rsh, &mut contents);
            } else {
                println!("Failed to read the file contents");
            }
        } else {
            println!("File not found: {:?}", var);
        }
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
                Node::ExecScript(script) => {
                    self.eval_exec_script(*script);
                }
                Node::Pipeline(pipeline) => {
                    // パイプライン処理
                    self.eval_pipeline(pipeline);
                }
                _ => {
                    println!("I don't know: {:?}", s);
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
        assert_eq!(result.unwrap(), Status::success());
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
