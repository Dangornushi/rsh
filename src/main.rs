mod command;
mod error;
mod log;

use crate::log::log_maneger::csv_reader;
use crate::log::log_maneger::csv_writer;
use crate::log::log_maneger::History;
use colored::Colorize;
use crossterm::cursor::MoveTo;
use crossterm::event::read;
use crossterm::event::KeyEvent;
use crossterm::{
    cursor::MoveLeft,
    cursor::MoveToColumn,
    event::{Event, KeyCode},
    execute,
    style::{Color, Print, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use error::error::{RshError, Status};
use nix::sys::wait::*;
use nix::{
    errno::Errno,
    sys::{
        signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal},
        wait::waitpid,
    },
    unistd::{close, execvp, fork, getpgrp, pipe, setpgid, tcsetpgrp, ForkResult},
};
use std::env;
use std::ffi::CString;
use std::fs;
use std::io::{stdout, Write};
use std::path;
use whoami::username;

struct Rsh {
    prompt: String,
    env_database: Vec<String>,
    history_database: Vec<History>,
    command_database: Vec<String>,
    return_code: i32,
}

impl Rsh {
    fn open_profile(&self, path: &str) -> Result<String, RshError> {
        let home_dir = env::var("HOME")
            .map_err(|_| env::var("."))
            .map_err(|_| RshError::new("Failed to get HOME directory"))?;
        Ok(format!("{}/{}", home_dir, path))
    }

    fn eprintln(&self, message: &str) {
        let mut stderr = std::io::stderr();
        std::io::stdout().flush().unwrap();
        execute!(stderr, Print("\n"), Print(message), Print("\n"))
            .map_err(|_| RshError::new("Failed to print error message"))
            .unwrap();

        std::io::stdout().flush().unwrap();
    }

    fn get_executable_commands(&mut self) {
        self.command_database.clear();
        if let Some(paths) = env::var_os("PATH") {
            for path in env::split_paths(&paths) {
                if let Ok(entries) = fs::read_dir(path) {
                    for entry in entries {
                        if let Ok(entry) = entry {
                            let path = entry.path();
                            if path.is_file() {
                                if let Some(file_name) = path.file_name() {
                                    if let Some(file_name_str) = file_name.to_str() {
                                        self.command_database.push(file_name_str.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            self.command_database.sort();
        }
    }

    fn get_directory_contents(&mut self, path: &str) {
        let mut contents = Vec::new();
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if let Some(file_name) = path.file_name() {
                        if let Some(file_name_str) = file_name.to_str() {
                            contents.push(file_name_str.to_string());
                        }
                    }
                }
            }
        }
        contents.sort();
        self.command_database.splice(0..0, contents);
    }

    fn get_rshenv_contents(&mut self) -> Result<(), RshError> {
        let rshenv_path = self.open_profile(".rshenv")?;
        let data =
            fs::read_to_string(&rshenv_path).map_err(|_| RshError::new("Failed to open rshenv"))?;
        self.env_database = data.lines().map(|line| line.to_string()).collect();

        Ok(())
    }

    fn get_rshhistory_contents(&mut self) -> Result<(), RshError> {
        let history_path = self.open_profile(".rsh_history")?;

        self.history_database =
            csv_reader(&history_path).map_err(|_| RshError::new("Failed to get history path"))?;
        Ok(())
    }

    fn get_current_dir_as_vec(&self) -> Vec<String> {
        let current_dir = std::env::current_dir().unwrap();
        let path = current_dir.as_path();
        let mut now_dir: Vec<String> = path
            .components()
            .map(|component| component.as_os_str().to_string_lossy().to_string())
            .collect();

        if now_dir.len() > 2 {
            now_dir.remove(0);
            now_dir.remove(0);
            now_dir.remove(0);
        }

        now_dir
    }

    fn rsh_char_search(
        &self,
        search_string: String,
        counter: &mut usize,
    ) -> Result<String, RshError> {
        let matches = self
            .command_database
            .iter()
            .filter(|command| command.starts_with(&search_string));

        let filtered_commands: Vec<String> = matches.map(|s| s.to_string()).collect();
        match filtered_commands.len() {
            0 => {
                return Err(RshError::new("No command found"));
            }
            _ => {
                if filtered_commands.clone().len() < *counter {
                    *counter = 0;
                }
                Ok(filtered_commands[*counter].clone())
            }
        }
    }

    fn set_prompt_color(&self, color_code: String) -> Result<(), RshError> {
        if color_code.len() != 7 || !color_code.starts_with('#') {
            return Err(RshError::new("Invalid color code"));
        }

        let r = u8::from_str_radix(&color_code[1..3], 16)
            .map_err(|_| RshError::new("Invalid red value"))?;
        let g = u8::from_str_radix(&color_code[3..5], 16)
            .map_err(|_| RshError::new("Invalid green value"))?;
        let b = u8::from_str_radix(&color_code[5..7], 16)
            .map_err(|_| RshError::new("Invalid blue value"))?;

        let mut stdout = stdout();
        execute!(stdout, SetForegroundColor(Color::Rgb { r, g, b }))
            .map_err(|_| RshError::new("Failed to set color"))?;

        Ok(())
    }

    fn set_prompt(&mut self) -> Result<(), RshError> {
        let mut stdout = stdout();
        // ui ----------------------------------------------------
        // Set the prompt color
        self.set_prompt_color("#674196".to_string())?;
        execute!(
            stdout,
            MoveToColumn(0),
            Clear(ClearType::UntilNewLine),
            Print(username().bold()),
            Print(" "),
        )
        .map_err(|_| RshError::new("Failed to print directory"))?;

        self.set_prompt_color("#eaf4fc".to_string())?;

        // Display the current directory in the prompt
        let dir_s = self.get_current_dir_as_vec();
        for dir in dir_s {
            execute!(stdout, Print(dir), Print("/"))
                .map_err(|_| RshError::new("Failed to print directory"))?;
        }

        execute!(stdout, Print(" ["), Print(self.return_code), Print("] > "))
            .map_err(|_| RshError::new("Failed to print directory"))?;

        std::io::stdout().flush().unwrap();
        // --------------------------------------------------------
        Ok(())
    }

    fn rsh_read_line(&mut self) -> String {
        let mut buffer = String::new();
        let mut stdout = stdout();
        let mut pushed_tab = false;
        let mut stack_buffer = String::new();
        let mut tab_counter = 0;
        let mut space_counter = 0;
        enable_raw_mode().unwrap();

        let _ = self.set_prompt();
        loop {
            self.get_directory_contents("./");

            // キー入力の取得
            if let Event::Key(KeyEvent {
                code,
                modifiers: _,
                kind: _,
                state: _,
            }) = read().unwrap()
            {
                match code {
                    KeyCode::Tab => {
                        if !pushed_tab {
                            // 現時点で入力されている文字のバックアップ
                            stack_buffer = buffer.clone();
                        }
                        // コマンドDBの取得
                        self.get_executable_commands();
                        self.get_directory_contents("./");

                        // 予測されるコマンドを取得
                        if let Ok(autocomplete) =
                            self.rsh_char_search(stack_buffer.clone(), &mut tab_counter)
                        {
                            buffer = autocomplete;
                        }

                        pushed_tab = true;
                        tab_counter += 1;
                    }
                    KeyCode::Enter => break,
                    _ => {
                        // TABの直後に文字が入力された場合
                        if pushed_tab {
                            // 予測変換をキャンセルさせる
                            buffer = stack_buffer.clone();
                            pushed_tab = false;
                            tab_counter = 0;
                        }
                        buffer = match code {
                            KeyCode::Backspace => {
                                if let Some(last_char) = buffer.chars().last() {
                                    if last_char == ' ' {
                                        space_counter -= 1;
                                    }
                                }
                                buffer.pop();
                                buffer.clone()
                            }
                            KeyCode::Char(' ') => {
                                space_counter += 1;
                                format!("{} ", buffer)
                            }
                            KeyCode::Char(c) => format!("{}{}", buffer, c),
                            _ => buffer,
                        };
                    }
                }
            }

            // キー入力がない場合
            // command_databaseの中からbufferで始まるものを取得
            let matches = self
                .command_database
                .iter()
                .filter(|command| command.starts_with(&buffer));

            // 上記を配列に変換
            let mut filtered_commands: Vec<String> = matches.map(|s| s.to_string()).collect();

            // もしもコマンドが見つからなかった場合
            if filtered_commands.len() == 0 {
                for env_path in self.env_database.clone() {
                    // command_databaseの中からenv_path/bufferで始まるものを取得
                    let matches = self.command_database.iter().filter(|command| {
                        let command_path = format!("{}/{}", env_path, buffer);
                        command.starts_with(&buffer) || command_path.starts_with(&buffer)
                    });

                    // 上記を配列に変換
                    filtered_commands = matches.map(|s| s.to_string()).collect();
                }
            }

            let _ = self.set_prompt();
            let print_buf = buffer.clone();
            let print_buf_parts: Vec<&str> = print_buf.split_whitespace().collect();

            let mut tmp = 0;
            for i in &print_buf_parts {
                execute!(stdout, Print(i)).unwrap();
                if tmp < space_counter {
                    execute!(stdout, Print(" ")).unwrap();
                }
                tmp += 1;
            }

            if filtered_commands.len() > 0 {
                // コマンドが見つかった場合
                // 予測変換の表示

                /* TAB押下時と同様 */
                if !pushed_tab {
                    // 現時点で入力されている文字のバックアップ
                    stack_buffer = buffer.clone();
                }
                // コマンドDBの取得
                self.get_executable_commands();
                self.get_directory_contents("./");

                // 予測されるコマンドを取得
                if let Ok(autocomplete) =
                    self.rsh_char_search(stack_buffer.clone(), &mut tab_counter)
                {
                    buffer = autocomplete;
                }

                pushed_tab = true;

                self.set_prompt_color("#9ea1a3".to_string()).unwrap();
                if print_buf.len() < buffer.len() {
                    execute!(
                        stdout,
                        // 予想される文字
                        Print(buffer[print_buf.len()..].to_string()),
                        // カーソルをbufferまで移動
                        MoveLeft(buffer[print_buf.len()..].to_string().len() as u16)
                    )
                    .unwrap();
                }
                std::io::stdout().flush().unwrap();
            } else {
                // 予測変換がない場合
                execute!(
                    stdout,
                    //SetForegroundColor(Color::Red),
                    SetForegroundColor(Color::White),
                )
                .unwrap();
                std::io::stdout().flush().unwrap();
            }
        }
        disable_raw_mode().unwrap();
        execute!(stdout, Print("\n")).unwrap();
        std::io::stdout().flush().unwrap();
        return buffer;
    }

    fn rsh_split_line(&self, line: String) -> Vec<String> {
        let mut quote_flag = false;
        let mut in_quote_buffer = String::new();
        let mut buffer = String::new();
        let mut r_vec = Vec::new();

        for c in line.chars() {
            if c == '"' {
                match quote_flag {
                    true => {
                        //閉じるクォート
                        buffer.push_str(&in_quote_buffer);
                        buffer.push('"');
                        in_quote_buffer.clear();
                    }
                    false => {
                        //始めるクォート
                        buffer.push('"');
                    }
                }
                quote_flag = !quote_flag;
            } else if c == ' ' && quote_flag != true {
                r_vec.push(buffer.clone());
                buffer.clear();
            } else {
                match quote_flag {
                    true => in_quote_buffer.push(c),
                    false => {
                        buffer.push(c);
                    }
                }
            }
        }
        r_vec.push(buffer.clone());
        buffer.clear();
        r_vec
    }

    fn ignore_tty_signals(&self) {
        let sa = SigAction::new(SigHandler::SigIgn, SaFlags::empty(), SigSet::empty());
        unsafe {
            sigaction(Signal::SIGTSTP, &sa).unwrap();
            sigaction(Signal::SIGTTIN, &sa).unwrap();
            sigaction(Signal::SIGTTOU, &sa).unwrap();
        }
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
        let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let path = self.open_profile(".rsh_history")?;

        csv_writer(args.join(" "), time, &path)
            .map_err(|_| RshError::new("Failed to write history"))?;

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
                        self.return_code = return_code;
                        Ok(Status::Success)
                    }
                    Ok(WaitStatus::Signaled(_, _, _)) => {
                        println!("signaled");
                        Ok(Status::Success)
                    }
                    Err(err) => {
                        self.eprintln(&format!("rsh: {}", err.message));
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
                    .map(|_| Status::Success)
                    .map_err(|_| RshError::new(&format!("{} is not found", args[0])))

                // -------------
            }
        }
    }

    fn rsh_execute(&mut self, args: Vec<String>) -> Result<Status, RshError> {
        if let Option::Some(arg) = args.get(0) {
            return match arg.as_str() {
                // cd: ディレクトリ移動の組み込みコマンド
                "cd" =>
                match
                command::cd::rsh_cd(if let Option::Some(dir) = args.get(1) {
                    dir
                } else {
                    execute!(stdout(), Print("\n")).unwrap();
                    std::io::stdout().flush().unwrap();
                    "./"
                }) {
                    Err(err) => {
                        self.eprintln(&format!("Error: {}", err.message));
                        Ok(Status::Success)
                    }
                    _ => Ok(Status::Success),

                }
                ,
                // ロゴ表示
                "%logo" => command::logo::rsh_logo(),
                // history: 履歴表示の組み込みコマンド
                "%fl" => command::history::rsh_history(self.history_database.clone()).map(|_| Status::Success),
                // exit: 終了用の組み込みコマンド
                "exit" => command::exit::rsh_exit(),
                // none: 何もなければコマンド実行
                _ => self.rsh_launch(args),
            };
        }
        Ok(Status::Success)
    }

    pub fn rsh_loop(&mut self) -> Result<Status, RshError> {
        let mut stdout = stdout();

        self.ignore_tty_signals();

        execute!(stdout, Print("\n"),)
            .map_err(|_| RshError::new("Failed to print directory"))
            .unwrap();

        std::io::stdout().flush().unwrap();

        // 絶対値なので相対移動になるようになんとかする
        let _ = execute!(stdout, MoveTo(0, 0), Clear(ClearType::All));

        loop {
            let line = self.rsh_read_line();
            let args = self.rsh_split_line(line);

            self.get_executable_commands();
            self.get_rshhistory_contents()?;
            self.get_rshenv_contents()?;

            match self.rsh_execute(args) {
                Ok(status) => match status {
                    Status::Success => continue,
                    exit @ Status::Exit => return Ok(exit),
                },
                err @ Err(_) => return err,
            };
        }
    }

    pub fn new() -> Self {
        Self {
            prompt: String::new(),
            env_database: Vec::new(),
            history_database: Vec::new(),
            command_database: Vec::new(),
            return_code: 0,
        }
    }
}

fn main() {
    let mut rsh = Rsh::new();
    let code = rsh.rsh_loop();
    println!("rsh: {:?}", code);
}
