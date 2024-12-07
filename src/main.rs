mod command;
mod error;

use colored::Colorize;
use crossterm::cursor::MoveRight;
use crossterm::cursor::MoveTo;
use crossterm::event::read;
use crossterm::event::KeyEvent;
use crossterm::{
    cursor,
    cursor::MoveLeft,
    cursor::MoveToColumn,
    event::{Event, KeyCode},
    execute,
    style::{Color, Print, SetForegroundColor},
    terminal,
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
use std::fmt::format;
use std::fs;
use std::io::{stdin, stdout, Read, Write};
use std::thread;
use std::time::Duration;
use whoami::username;

struct Autcomplete {
    buffer: String,
    exit: bool,
}
struct Rsh {
    prompt: String,
    command_database: Vec<String>,
}

impl Rsh {
    fn get_executable_commands(&mut self) {
        let mut commands: Vec<String> = Vec::new();
        let mut files: Vec<String> = Vec::new();

        self.command_database.clear();
        if let Ok(entries) = fs::read_dir(env::current_dir().unwrap()) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if let Some(file_name) = path.file_name() {
                        if let Some(file_name_str) = file_name.to_str() {
                            files.push(file_name_str.to_string());
                        }
                    }
                }
            }
        }
        if let Some(paths) = env::var_os("PATH") {
            for path in env::split_paths(&paths) {
                if let Ok(entries) = fs::read_dir(path) {
                    for entry in entries {
                        if let Ok(entry) = entry {
                            let path = entry.path();
                            if path.is_file() {
                                if let Some(file_name) = path.file_name() {
                                    if let Some(file_name_str) = file_name.to_str() {
                                        commands.push(file_name_str.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            commands.sort();
        }
        files.extend(commands);
        self.command_database = files;
    }

    fn rsh_char_search(
        &self,
        search_string: String,
        counter: &mut usize,
    ) -> Result<String, RshError> {
        let matches: Vec<&String> = self
            .command_database
            .iter()
            .filter(|command| command.starts_with(&search_string))
            .collect();
        match matches.len() {
            0 => return Err(RshError::new("no matches")),
            _ => {}
        }
        let filtered_commands: Vec<String> = matches.iter().map(|s| s.to_string()).collect();
        if *counter >= filtered_commands.len() {
            *counter = 0;
            return Ok(filtered_commands[*counter].clone());
        }
        Ok(filtered_commands[*counter].clone())
    }

    fn join_with_spaces(&self, vec: Vec<String>) -> String {
        let mut r_string = String::new();
        for i in 0..vec.len() {
            r_string = format!("{}{} ", r_string, vec[i]);
        }
        r_string
    }

    fn rsh_read_line(&mut self) -> String {
        let mut buffer = String::new();
        let mut stdout = stdout();
        let mut pushed_tab = false;
        let mut stack_buffer = String::new();
        let mut tab_counter = 0;
        let mut space_counter = 0;

        let mut input_buffer: Vec<String> = Vec::new();
        let mut out_buffer = String::new();
        let mut ghost_auto_complete = false;

        enable_raw_mode().unwrap();

        loop {
            if let Event::Key(KeyEvent {
                code,
                modifiers: _,
                kind: _,
                state: _,
            }) = read().unwrap()
            {
                // コマンドDBの取得
                self.get_executable_commands();

                if self
                    .command_database
                    .binary_search_by(|cmd| cmd.as_str().cmp(&buffer))
                    .is_ok()
                {
                    ghost_auto_complete = true;
                } else {
                    ghost_auto_complete = false;
                }
                match code {
                    KeyCode::Tab => {
                        if !pushed_tab {
                            // 現時点で入力されている文字のバックアップ
                            stack_buffer = buffer.clone();
                        }
                        // コマンドDBの取得
                        //        self.get_executable_commands();

                        // 予測されるコマンドを取得
                        if let Ok(autocomplete) =
                            self.rsh_char_search(stack_buffer.clone(), &mut tab_counter)
                        {
                            buffer = autocomplete;
                        }

                        pushed_tab = true;
                        tab_counter += 1;
                    }
                    KeyCode::Backspace => {
                        if pushed_tab {
                            // 予測変換をキャンセルさせる
                            buffer = stack_buffer.clone();
                            pushed_tab = false;
                            tab_counter = 0;
                        } else {
                            // input_bufferもbufferもからの場合は何もしない
                            if buffer.len() == 0 && input_buffer.len() == 0 {
                                continue;
                            }
                            // 削除された文字がスペースであればinput_iterを戻す
                            if buffer.ends_with(' ') {
                                space_counter -= 1;
                                input_buffer.pop();
                            } else if buffer.len() == 0 {
                                // bufferが空の場合はinput_bufferから取り出す
                                buffer = input_buffer.pop().unwrap();
                                space_counter -= 1;
                            } else {
                                buffer.pop();
                            }
                        }
                    }
                    KeyCode::Enter => {
                        /*
                                                if ghost_auto_complete {
                                                    buffer = stack_buffer.clone();
                                                    ghost_auto_complete = false;
                                                    ghost_auto_complete = true;
                                                } else {
                        */
                        break;
                        //                        }
                    }
                    KeyCode::Char(' ') => {
                        input_buffer.push(buffer.clone());
                        space_counter += 1;
                        buffer.clear();
                    }
                    _ => {
                        // TABの直後に文字が入力された場合
                        if pushed_tab {
                            // 予測変換をキャンセルさせる
                            buffer = stack_buffer.clone();
                            pushed_tab = false;
                            tab_counter = 0;
                        }
                        match code {
                            KeyCode::Backspace => {
                                // 削除された文字がスペースであればinput_iterを戻す
                                if buffer.ends_with(' ') {
                                    space_counter -= 1;
                                    input_buffer.pop();
                                } else {
                                    buffer.pop();
                                }
                            }
                            KeyCode::Char(c) => {
                                buffer = format!("{}{}", buffer, c);
                            }
                            _ => {}
                        };
                    }
                }
            }
            execute!(
                stdout,
                MoveToColumn(0),
                SetForegroundColor(Color::White),
                Clear(ClearType::UntilNewLine),
                Print(self.prompt.clone()),
            )
            .unwrap();
            out_buffer = format!(
                "{}{}",
                if input_buffer.len() > 0 {
                    self.join_with_spaces(input_buffer.clone())
                } else {
                    "".to_string()
                },
                buffer
            );

            if ghost_auto_complete {
                // 色を変えて再度出力
                execute!(
                    stdout,
                    SetForegroundColor(Color::Red),
                    Print(out_buffer.clone()),
                )
                .unwrap();
                std::io::stdout().flush().unwrap();
            } else {
                execute!(stdout, Print(out_buffer.clone()),).unwrap();
                std::io::stdout().flush().unwrap();
            }
        }
        disable_raw_mode().unwrap();
        return out_buffer;
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

    fn rsh_launch(&self, args: Vec<String>) -> Result<Status, RshError> {
        print!("\n");
        std::io::stdout().flush().unwrap();

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
                        //println!("Exited: {}", return_code);
                        Ok(Status::Success)
                    }
                    Ok(WaitStatus::Signaled(_, _, _)) => {
                        println!("signaled");
                        Ok(Status::Success)
                    }
                    Err(err) => Err(RshError::new(&err.message)),
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
                    .map_err(|_| RshError::new(&format!("{} not found", args[0])))

                // -------------
            }
        }
    }

    fn rsh_cursor_test(&self) -> Result<(), std::io::Error> {
        let stdin = stdin();
        let buffer = [0];
        let mut rgb = 0;
        let mut counter = 0;

        terminal::enable_raw_mode()?;

        // 文字の出力
        execute!(stdout(), Print("Hello, world!"))?;

        loop {
            thread::sleep(Duration::from_millis(1));
            // カーソルを先頭に移動し、文字を消去
            execute!(stdout(), cursor::MoveToColumn(1))?; //, cursor::MoveToNextLine(1))?;

            // 色を変えて再度出力
            execute!(
                stdout(),
                SetForegroundColor(Color::Rgb { r: rgb, g: 0, b: 0 }),
                Print("Hello, world!")
            )?;

            if rgb > 254 {
                rgb = 0;
            } else {
                rgb += 1;
            }

            if counter > 254 * 5 {
                break;
            }
            counter += 1;
        }
        // 元の状態に戻す
        terminal::disable_raw_mode()?;

        Ok(())
    }

    fn rsh_execute(&self, args: Vec<String>) -> Result<Status, RshError> {
        if let Option::Some(arg) = args.get(0) {
            return match arg.as_str() {
                // cd: ディレクトリ移動の組み込みコマンド
                "cd" => {
                    let r_code = command::cd::rsh_cd(if let Option::Some(dir) = args.get(1) {
                        dir
                    } else {
                        ""
                    });
                    print!("\n");
                    std::io::stdout().flush().unwrap();
                    r_code
                }
                // ロゴ表示
                "%logo" => command::logo::rsh_logo(),
                "%" => {
                    let _ = self.rsh_cursor_test();
                    Ok(Status::Success)
                }
                // exit: 終了用の組み込みコマンド
                "exit" => command::exit::rsh_exit(),
                // none: 何もなければコマンド実行
                _ => self.rsh_launch(args),
            };
        }
        Ok(Status::Success)
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

    pub fn rsh_loop(&mut self) -> Result<Status, RshError> {
        self.prompt = ">".to_string();
        let mut stdout = stdout();

        self.ignore_tty_signals();

        // 絶対値なので相対移動になるようになんとかする
        let _ = execute!(stdout, MoveTo(0, 0), Clear(ClearType::All));

        loop {
            // ui ------------------------------------------------------------------
            self.prompt = format!("{}: ", username().green().bold());

            // 文字色処理アルゴリズム ---------------------------------
            let dir_s = self.get_current_dir_as_vec();
            for i in dir_s {
                //print!("{}/", i.white().bold()); //.custom_color(path_base_color));
                self.prompt = format!("{}{}/", self.prompt, i.white().bold());
            }
            self.prompt = format!("{} > ", self.prompt);

            print!("{}", self.prompt);

            // --------------------------------------------------------

            std::io::stdout().flush().unwrap();
            let line = self.rsh_read_line();
            let args = self.rsh_split_line(line);

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
            command_database: Vec::new(),
        }
    }
}

fn main() {
    let mut rsh = Rsh::new();
    let code = rsh.rsh_loop();
    println!("> {:?}", code);
}
