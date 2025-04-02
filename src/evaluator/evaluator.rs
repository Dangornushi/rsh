use crate::command;
use crate::error::error::{RshError, Status, StatusCode};
use crate::parser::parse::{
    CommandStatement, CompoundStatement, Define, ExecScript, Identifier, Node, Pipeline, Redirect,
    RedirectErrorOutput, RedirectErrorOutputAppend, RedirectInput, RedirectOutput,
    RedirectOutputAppend, Reference,
};
use crate::rsh::rsh::Rsh;
use nix::libc;
use nix::sys::wait::wait;
use nix::unistd::{close, dup2, fork, pipe, ForkResult};
use std::ffi::CString;
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::os::unix::io::RawFd;

use crossterm::{execute, style::Print};
use std::any::Any;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::{stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// リダイレクト
#[derive(Debug, Clone)]
enum OutputOption {
    Append,
    Overwrite,
}
#[derive(Debug, Clone)]
pub struct OutputBool {
    option: OutputOption,
    is_enable: bool,
}
impl OutputBool {
    fn new() -> OutputBool {
        OutputBool {
            option: OutputOption::Overwrite,
            is_enable: false,
        }
    }
    fn enable(&mut self, option: OutputOption) {
        self.is_enable = true;
        self.option = option
    }
}
#[derive(Debug, Clone)]
struct RedirectFD {
    input: String,
    output: String,
    error: String,

    pub do_redirect_input: bool,
    pub do_redirect_output: OutputBool,
    pub do_redirect_error: OutputBool,
}

impl RedirectFD {
    fn new() -> RedirectFD {
        RedirectFD {
            input: String::new(),
            output: String::new(),
            error: String::new(),
            do_redirect_input: false,
            do_redirect_output: OutputBool::new(),
            do_redirect_error: OutputBool::new(),
        }
    }

    pub fn input(&self) {
        let file = File::open(self.input.clone()).expect("Failed to create output file");
        let fd = file.as_raw_fd();

        // Redirect stdin to the given file descriptor
        if let Err(err) = dup2(fd, libc::STDIN_FILENO) {
            eprintln!("Failed to redirect input: {}", err);
        }
    }

    pub fn output(&self) {
        if !self.do_redirect_output.is_enable {
            return;
        }

        let f = match self.do_redirect_output.option.clone() {
            OutputOption::Append => {
                // Open a file to use as output
                OpenOptions::new()
                    .create(true) // ファイルが存在しない場合は作成
                    .append(true) // 既存の内容に追加
                    .open(self.output.clone())
                    .expect("Failed to open output file in append mode")
            }
            OutputOption::Overwrite => {
                // Open a file to use as output
                File::create(self.output.clone()).expect("Failed to create output file")
            }
        };
        let fd = f.as_raw_fd();

        // Redirect stderr to the given file descriptor
        if let Err(err) = dup2(fd, libc::STDOUT_FILENO) {
            eprintln!("Failed to redirect error output: {}", err);
        }
    }

    pub fn error(&self) {
        if !self.do_redirect_error.is_enable {
            return;
        }

        let f = match self.do_redirect_error.option.clone() {
            OutputOption::Append => {
                // Open a file to use as output
                OpenOptions::new()
                    .create(true) // ファイルが存在しない場合は作成
                    .append(true) // 既存の内容に追加
                    .open(self.error.clone())
                    .expect("Failed to open output file in append mode")
            }
            OutputOption::Overwrite => {
                // Open a file to use as output
                File::create(self.error.clone()).expect("Failed to create output file")
            }
        };
        let fd = f.as_raw_fd();

        // Redirect stderr to the given file descriptor
        if let Err(err) = dup2(fd, libc::STDERR_FILENO) {
            eprintln!("Failed to redirect error output: {}", err);
        }
    }
}

impl Drop for RedirectFD {
    fn drop(&mut self) {
        // 標準出力を元に戻す
        if let Err(err) = dup2(libc::STDOUT_FILENO, libc::STDOUT_FILENO) {
            eprintln!("Failed to reset output: {}", err);
        }

        // 標準エラー出力を元に戻す
        if let Err(err) = dup2(libc::STDERR_FILENO, libc::STDERR_FILENO) {
            eprintln!("Failed to reset error output: {}", err);
        }

        // 標準入力を元に戻す
        if let Err(err) = dup2(libc::STDIN_FILENO, libc::STDIN_FILENO) {
            eprintln!("Failed to reset input: {}", err);
        }

        self.input.clear();
        self.output.clear();
        self.error.clear();

        self.do_redirect_input = false;
        self.do_redirect_output = OutputBool::new();
        self.do_redirect_error = OutputBool::new();
    }
}
// -------------------------------------------------------------

#[derive(Debug, PartialEq, Clone)]
pub struct Variable {
    name: String,
    value: String,
}
impl Variable {
    pub fn new(name: String, value: String) -> Self {
        Variable { name, value }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Function {
    name: String,
    body: Node,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Memory {
    variables: HashMap<String, Variable>,
    functions: HashMap<String, Function>,
    exit_code: i32,
}
impl Memory {
    pub fn push(&mut self, variable: Variable) {
        self.variables.insert(variable.name.clone(), variable);
    }
    pub fn new() -> Self {
        Memory {
            variables: HashMap::new(),
            functions: HashMap::new(),
            exit_code: 0,
        }
    }
}
pub struct Evaluator {
    rsh: Rsh,
    memory: Memory,
    redirect: RedirectFD,
}

impl Evaluator {
    pub fn new(rsh: Rsh) -> Self {
        Evaluator {
            rsh,
            memory: Memory::new(),
            redirect: RedirectFD::new(),
        }
    }

    fn setup_signal_handler(&self) -> Arc<AtomicBool> {
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        // Set up a signal handler for SIGINT (Ctrl+C)
        signal_hook::flag::register(signal_hook::consts::SIGINT, r.clone())
            .expect("Failed to register SIGINT handler");

        running
    }

    fn run(&self, commands: Vec<Vec<String>>, redirect: RedirectFD) -> Result<Status, RshError> {
        // 組み込みコマンドの実行
        if commands.is_empty() {
            return Ok(Status::success());
        }

        match commands[0][0].as_str() {
            // cd: ディレクトリ移動の組み込みコマンド
            "cd" => {
                match command::cd::rsh_cd(if let Option::Some(dir) = commands[0].get(1) {
                    dir
                } else {
                    execute!(stdout(), Print("\n")).unwrap();
                    std::io::stdout().flush().unwrap();
                    "./"
                }) {
                    Err(err) => {
                        self.rsh.eprintln(&format!("Error: {}", err.message));
                        return Ok(Status::success());
                    }
                    _ => return Ok(Status::success()),
                }
            }
            // ロゴ表示
            "%logo" => return command::logo::rsh_logo(),
            // history: 履歴表示の組み込みコマンド
            "%fl" => {
                return command::history::rsh_history(self.rsh.get_history_database())
                    .map(|_| Status::success())
            }
            // exit: 終了用の組み込みコマンド
            "exit" => return command::exit::rsh_exit(),
            // none: 何もなければコマンド実行
            _ => {}
        };
        // それ以外のコマンドのための処理
        let pipe_count = commands.len() - 1;

        let mut pfd: Vec<(RawFd, RawFd)> = Vec::new();

        for _ in 0..pipe_count {
            pfd.push(pipe().expect("Failed to create pipe"));
        }

        // コマンドたちの解析
        for i in 0..=pipe_count {
            // コマンドの実行
            match fork() {
                Ok(ForkResult::Child) => {
                    redirect.error();
                    // Child process
                    if i == 0 && redirect.do_redirect_input {
                        // First command, no input redirection
                        redirect.input();
                    }
                    if i == pipe_count {
                        redirect.output();
                    }
                    if i < pipe_count {
                        // 今のコマンドの出力をパイプに設定
                        dup2(pfd[i].1, 1).expect("Failed to duplicate file descriptor");
                    }
                    if i > 0 {
                        dup2(pfd[i - 1].0, 0).expect("Failed to duplicate file descriptor");
                    }

                    // Close all pipe file descriptors
                    for &(read_fd, write_fd) in &pfd {
                        close(read_fd).ok();
                        close(write_fd).ok();
                    }

                    // Execute the command
                    let cmd =
                        CString::new(commands[i][0].as_str()).expect("Failed to create CString");
                    let args: Vec<CString> = commands[i]
                        .iter()
                        .map(|arg| CString::new(arg.as_str()).expect("Failed to create CString"))
                        .collect();

                    match nix::unistd::execvp(&cmd, &args) {
                        Err(err) => {
                            eprintln!("Command not found -> '{}' is {}", commands[i][0], err)
                        }
                        Ok(_) => {}
                    }
                }
                Ok(ForkResult::Parent { .. }) => {
                    // Parent process
                    // 実行したコマンドがパイプの終端ではない
                    if i < pipe_count {
                        // Close the write end of the current pipe
                        // 今のコマンドの出力を閉じる
                        close(pfd[i].1).ok();
                    }
                    // 実行したコマンドがパイプの始端ではない
                    if i > 0 {
                        // 前のコマンドの入力と出力を閉じる
                        close(pfd[i - 1].0).ok();
                        close(pfd[i - 1].1).ok();
                    }
                }
                Err(_) => {
                    eprintln!("Fork failed");
                    std::process::exit(1);
                }
            };
        }

        // Close remaining pipe file descriptors in the parent
        for &(read_fd, write_fd) in &pfd {
            close(read_fd).ok();
            close(write_fd).ok();
        }

        // Wait for all child processes to finish
        for _ in 0..=pipe_count {
            wait().ok();
        }

        Ok(Status::success())
    }

    fn eval_identifier(&self, expr: Identifier) -> String {
        //format!("\"{}\"", expr.eval())
        expr.eval()
    }

    fn eval_reference(&self, expr: Reference) -> Result<String, RshError> {
        let value = match expr.get_reference() {
            Node::Identifier(identifier) => {
                let eval_result = self.eval_identifier(identifier);
                eval_result.clone()
            }
            _ => String::new(),
        };
        if let Some(v) = self.memory.variables.get(&value) {
            Ok(v.value.clone())
        } else {
            Err(RshError::new("Failed to get value"))
        }
    }

    fn command_statement_to_vec(&self, expr: CommandStatement) -> Result<Vec<String>, RshError> {
        let command = match expr.get_command() {
            Node::Identifier(identifier) => self.eval_identifier(identifier.clone()),
            _ => return Err(RshError::new("Failed to get main command")),
        };
        let sub_command = expr
            .get_sub_command()
            .into_iter()
            .map(|node| match node {
                Node::Identifier(identifier) => Ok(identifier.eval()),
                Node::Reference(reference) => self.eval_reference(*reference),
                _ => Err(RshError::new("Failed to get sub command")),
            })
            .filter_map(|result| result.ok())
            .collect::<Vec<String>>();

        let mut full_command = vec![command.clone()];
        full_command.extend(sub_command);
        Ok(full_command)
    }

    fn eval_command(&mut self, expr: CommandStatement) -> Result<(), RshError> {
        let full_command = self.command_statement_to_vec(expr)?;

        // 分割したコマンドを実行
        let running = self.setup_signal_handler();
        let mut result = Status::new(StatusCode::Success, 0);
        while running.load(Ordering::SeqCst) {
            match self.run(vec![full_command.clone()], self.redirect.clone()) {
                Ok(r) => {
                    result = r;
                }
                Err(err) => {
                    println!("command:'{:?}' is {}", full_command, err.message);
                }
            }
            break;
        }

        if result.get_status_code() == StatusCode::Exit {
            std::process::exit(result.get_exit_code());
        }
        let _ = result.get_exit_code();

        Ok(())
    }

    fn eval_define(&mut self, define: Define) {
        let var = match define.get_var() {
            Node::Identifier(identifier) => self.eval_identifier(identifier),
            _ => Default::default(), // Handle other cases appropriately
        };
        let data = match define.get_data() {
            Node::Reference(reference) => self.eval_reference(*reference),
            Node::Identifier(identifier) => Ok(self.eval_identifier(identifier)),
            _ => Err(RshError::new("Failed to get data")),
        };

        if let Ok(data) = data {
            self.memory.push(Variable::new(var, data));
        }
    }

    fn eval_pipeline(&mut self, pipeline: Pipeline) {
        // パイプライン処理
        let mut commands = Vec::new();

        for command in pipeline.get_commands() {
            match command {
                Node::CommandStatement(command) => {
                    if let Ok(command) = self.command_statement_to_vec(*command) {
                        commands.push(command);
                    }
                }
                Node::Redirect(redirect) => {
                    self.eval_redirect_branch(redirect.get_destination());
                    if let Ok(command) = match redirect.get_command() {
                        Node::CommandStatement(command) => {
                            self.command_statement_to_vec(*command.clone())
                        }
                        _ => Err(RshError::new(
                            "Expected CommandStatement, found other Node type",
                        )),
                    } {
                        commands.push(command);
                    }
                }
                _ => {
                    println!("pipeline < I don't know: {:?}", command);
                }
            }
        }

        // 分割したコマンドを実行
        let running = self.setup_signal_handler();
        let mut result = Status::new(StatusCode::Success, 0);
        while running.load(Ordering::SeqCst) {
            match self.run(commands.clone(), self.redirect.clone()) {
                Ok(r) => {
                    result = r;
                }
                Err(err) => {
                    println!("command:'{:?}' is {}", commands, err.message);
                }
            }
            break;
        }

        if result.get_status_code() == StatusCode::Exit {
            std::process::exit(result.get_exit_code());
        }
        let _ = result.get_exit_code();
    }

    fn eval_redirect_input(&mut self, input: RedirectInput) {
        // リダイレクト処理
        self.redirect.input = match input.get_destination() {
            Node::Identifier(identifier) => self.eval_identifier(identifier),
            _ => {
                println!("redirect error: {:?}", input);
                unreachable!()
            }
        };
        self.redirect.do_redirect_input = true;
    }

    fn eval_redirect_output(&mut self, input: RedirectOutput) {
        // リダイレクト処理
        self.redirect.output = match input.get_destination() {
            Node::Identifier(identifier) => self.eval_identifier(identifier),
            _ => {
                println!("redirect error: {:?}", input);
                unreachable!()
            }
        };
        self.redirect
            .do_redirect_output
            .enable(OutputOption::Overwrite);
    }

    fn eval_redirect_output_append(&mut self, input: RedirectOutputAppend) {
        // リダイレクト処理
        self.redirect.output = match input.get_destination() {
            Node::Identifier(identifier) => self.eval_identifier(identifier),
            _ => {
                println!("redirect error: {:?}", input);
                unreachable!()
            }
        };
        self.redirect
            .do_redirect_output
            .enable(OutputOption::Append);
    }

    fn eval_redirect_error_output(&mut self, input: RedirectErrorOutput) {
        // リダイレクト処理
        self.redirect.error = match input.get_destination() {
            Node::Identifier(identifier) => self.eval_identifier(identifier),
            _ => {
                println!("redirect error: {:?}", input);
                unreachable!()
            }
        };
        self.redirect
            .do_redirect_error
            .enable(OutputOption::Overwrite);
    }

    fn eval_redirect_error_output_append(&mut self, input: RedirectErrorOutputAppend) {
        // リダイレクト処理
        self.redirect.error = match input.get_destination() {
            Node::Identifier(identifier) => self.eval_identifier(identifier),
            _ => {
                println!("redirect error: {:?}", input);
                unreachable!()
            }
        };
        self.redirect.do_redirect_error.enable(OutputOption::Append);
    }

    // Redirect構造体にファイル名を格納、Self.runの際にインスタンスを渡す
    fn eval_redirect_branch(&mut self, destinations: Vec<Node>) -> impl Any {
        // リダイレクト処理

        for destination in destinations {
            match destination {
                Node::RedirectInput(destination) => {
                    self.eval_redirect_input(*destination.clone());
                }
                Node::RedirectOutput(destination) => {
                    self.eval_redirect_output(*destination.clone());
                }
                Node::RedirectOutputAppend(destination) => {
                    self.eval_redirect_output_append(*destination.clone());
                }
                Node::RedirectErrorOutput(destination) => {
                    self.eval_redirect_error_output(*destination.clone());
                }
                Node::RedirectErrorOutputAppend(destination) => {
                    self.eval_redirect_error_output_append(*destination.clone());
                }
                _ => println!("other: {:?}", destination), // Handle other cases appropriately
            };
        }
        /**/
    }

    fn eval_redirect(&mut self, input: Redirect) {
        self.eval_redirect_branch(input.get_destination());
        let _ = self
            .eval_command(match input.get_command() {
                Node::CommandStatement(command) => *command,
                _ => {
                    unreachable!()
                }
            })
            .map_err(|err| {
                println!("Error: {:?}", err);
            });
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
                let _ = Rsh::execute_commands(&mut self.rsh, &mut contents);
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
                    let _ = self.eval_command(*command);
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
                Node::Redirect(redirect) => {
                    // リダイレクト処理
                    self.eval_redirect(*redirect);
                }
                Node::Comment(_) => {}
                _ => {
                    println!("compound_statement < I don't know: {:?}", s);
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
impl Drop for Evaluator {
    fn drop(&mut self) {
        // 標準出力を元に戻す
        if let Err(err) = dup2(libc::STDOUT_FILENO, libc::STDOUT_FILENO) {
            eprintln!("Failed to reset output: {}", err);
        }

        // 標準エラー出力を元に戻す
        if let Err(err) = dup2(libc::STDERR_FILENO, libc::STDERR_FILENO) {
            eprintln!("Failed to reset error output: {}", err);
        }

        // 標準入力を元に戻す
        if let Err(err) = dup2(libc::STDIN_FILENO, libc::STDIN_FILENO) {
            eprintln!("Failed to reset input: {}", err);
        }
    }
}
