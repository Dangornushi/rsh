mod command;
mod error;
mod log;

use crate::log::log_maneger::csv_reader;
use crate::log::log_maneger::csv_writer;
use crate::log::log_maneger::History;
use colored::Colorize;
use crossterm::cursor::MoveRight;
use crossterm::cursor::MoveTo;
use crossterm::event::read;
use crossterm::event::KeyEvent;
use crossterm::{
    cursor::MoveLeft,
    cursor::MoveToColumn,
    event::{Event, KeyCode},
    execute,
    style::{Color, Print, SetBackgroundColor, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use error::error::{RshError, Status};
use nix::libc;
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
use unicode_segmentation::UnicodeSegmentation;
use whoami::username;

struct Prompt {
    username: String,
    pwd: String,
    utils: String,
}

impl Prompt {
    pub fn new(username: String, pwd: Vec<String>, return_code: i32, mode: Mode) -> Self {
        let mode_str = match mode {
            Mode::Nomal => "N",
            Mode::Input => "I",
            Mode::Visual => "V",
            _ => "Else",
        };

        Self {
            username: format!("{} ", username),
            pwd: {
                let mut full_path = String::new();
                for dir in pwd {
                    full_path = format!("{}{}/", dir, full_path);
                }

                full_path
            },
            utils: format!(" [{}: {}] > ", return_code, mode_str),
        }
    }

    pub fn get_username(&self) -> String {
        self.username.clone()
    }
    pub fn get_pwd(&self) -> String {
        self.pwd.clone()
    }
    pub fn get_utils(&self) -> String {
        self.utils.clone()
    }

    pub fn len(&self) -> usize {
        self.username.len() + self.pwd.len() + self.utils.len()
    }
}

#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Nomal,
    Visual,
    Input,
}

struct Buffer {
    buffer: String,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }
}

#[derive()]
struct Rsh {
    prompt: String,
    buffer: Buffer,
    env_database: Vec<String>,
    history_database: Vec<History>,
    command_database: Vec<String>,
    return_code: i32,
    exists_rshenv: bool,
    now_mode: Mode,
    cursor_x: usize,
    char_count: usize,
}

impl Rsh {
    fn open_profile(&self, path: &str) -> Result<String, RshError> {
        let home_dir = env::current_dir()
            .unwrap()
            .into_os_string()
            .into_string()
            .map_err(|_| env::var("HOME"))
            .map_err(|_| env::var("."))
            .map_err(|_| RshError::new("Failed to get HOME directory"))?;
        Ok(format!("{}/{}", home_dir, path))
        /*
         */
    }

    fn eprintln(&self, message: &str) {
        let mut stderr = std::io::stderr();
        execute!(stderr, Print(message), Print("\n"))
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

        //self.println(&rshenv_path.clone());
        let data =
            fs::read_to_string(&rshenv_path).map_err(|_| RshError::new("Failed to open rshenv"))?;
        self.env_database = data.lines().map(|line| line.to_string()).collect();
        self.exists_rshenv = true;
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

    fn get_mode_string(&self) -> &str {
        match self.now_mode {
            Mode::Nomal => "N",
            Mode::Input => "I",
            Mode::Visual => "V",
            _ => "Else",
        }
    }

    fn set_prompt(&mut self) -> Result<(), RshError> {
        let mut stdout = stdout();
        // ui ----------------------------------------------------
        // Set the prompt color
        if self.exists_rshenv {
            // Theme
            // 環境変数設定ファイルが存在する
            self.set_prompt_color("#AC6683".to_string())?;
        } else {
            // Theme
            self.set_prompt_color("#A61602".to_string())?;
        }
        execute!(
            stdout,
            MoveToColumn(0),
            Clear(ClearType::UntilNewLine),
            Print(username().bold()),
            Print(" "),
        )
        .map_err(|_| RshError::new("Failed to print directory"))?;

        // Theme
        self.set_prompt_color("#d1d1d1".to_string())?;

        // Display the current directory in the prompt
        let dir_s = self.get_current_dir_as_vec();
        for dir in dir_s {
            execute!(stdout, Print(dir), Print("/"))
                .map_err(|_| RshError::new("Failed to print directory"))?;
        }

        // Theme
        self.set_prompt_color("#f8f8f8".to_string())?;
        execute!(stdout, Print(" [".to_string())).unwrap();
        self.set_prompt_color("#589F62".to_string())?;
        execute!(stdout, Print(self.return_code)).unwrap();
        self.set_prompt_color("#fafafa".to_string())?;
        execute!(stdout, Print(": ".to_string())).unwrap();

        match self.now_mode {
            // Theme
            Mode::Input => self.set_prompt_color("#218587".to_string())?,
            Mode::Nomal => self.set_prompt_color("#589F62".to_string())?,
            Mode::Visual => self.set_prompt_color("#E9B42C".to_string())?,
        }
        execute!(stdout, Print(self.get_mode_string())).unwrap();

        // Theme
        self.set_prompt_color("#fafafa".to_string())?;
        execute!(stdout, Print("] > ")).unwrap();

        //std::io::stdout().flush().unwrap();
        // --------------------------------------------------------
        Ok(())
    }

    fn set_mode(&mut self, mode: Mode) {
        self.now_mode = mode;
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

    fn rsh_char_search(
        &self,
        search_string: String,
        counter: &mut usize,
    ) -> Result<String, RshError> {
        let matches = self
            .command_database
            .iter()
            .filter(|command| command.starts_with(&search_string));

        let history_matches: Vec<String> = self
            .history_database
            .iter()
            .filter(|history| history.get_command().starts_with(&search_string))
            .map(|history| history.get_command().to_string())
            .collect();

        let mut filtered_commands: Vec<String> =
            history_matches.into_iter().map(|s| s.to_string()).collect();
        filtered_commands.extend(matches.map(|s| s.to_string()));

        match filtered_commands.len() {
            0 => {
                return Err(RshError::new("No command found"));
            }
            _ => {
                if filtered_commands.clone().len() <= *counter {
                    *counter -= 1;
                }
                Ok(filtered_commands[*counter].clone())
            }
        }
    }

    fn rsh_split_line(&self, line: String) -> Vec<String> {
        let mut quote_flag = false;
        let mut in_quote_buffer = String::new();
        let mut buffer = String::new();
        let mut r_vec = Vec::new();
        let mut quote_start_index = 0;

        for c in line.chars() {
            if c == '"' {
                match quote_flag {
                    true => {
                        //クォートに囲まれた文字列を挿入
                        buffer.replace_range(quote_start_index.., &in_quote_buffer);

                        //閉じるクォートを挿入
                        buffer.push('"');
                        in_quote_buffer.clear();
                        quote_start_index = 0;
                    }
                    false => {
                        //始めるクォート
                        buffer.push('"');
                        // クォートが閉じられた際に挿入される部分を記録
                        quote_start_index = buffer.len();
                    }
                }
                quote_flag = !quote_flag;
            } else if c == ' ' && quote_flag != true {
                // Bufferが空の場合はスペースを挿入する
                r_vec.push(buffer.clone());
                buffer.clear();
            } else {
                buffer.push(c);
                match quote_flag {
                    true => in_quote_buffer.push(c),
                    false => {}
                }
            }
        }
        r_vec.push(buffer.clone());
        buffer.clear();
        r_vec
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
                    .map(|_| Status::Success)
                    .map_err(|_| RshError::new(&format!("{} is not found", args[0])))

                // -------------
            }
        }
    }

    fn rsh_execute(&mut self, args: Vec<String>) -> Result<Status, RshError> {
        if let Option::Some(arg) = args.get(0) {
            let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let path = self.open_profile(".rsh_history")?;

            csv_writer(args.join(" "), time, &path)
                .map_err(|_| RshError::new("Failed to write history"))?;
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

    pub fn rsh_print(&self, buffer: String) {
        let space_counter = buffer.chars().filter(|&c| c == ' ').count();
        let print_buf_parts: Vec<String> = self.rsh_split_line(buffer.clone()); //print_buf.split_whitespace().collect();

        let mut tmp = 0;
        // 瓶覗 かめのぞき
        // コマンドの色
        self.set_prompt_color("#457E7D".to_string()).unwrap();
        for i in &print_buf_parts {
            execute!(stdout(), Print(i)).unwrap();
            if tmp < space_counter {
                tmp += 1;
                execute!(stdout(), Print(" ")).unwrap();
                // コマンド引数の色
                self.set_prompt_color("#809E8A".to_string()).unwrap();
            }
        }
    }

    pub fn get_string_at_cursor(&self, start_pos: usize) -> String {
        self.buffer
            .buffer
            .chars()
            .enumerate()
            .filter(|(i, _)| {
                if start_pos < self.cursor_x {
                    *i < start_pos || *i > self.cursor_x
                } else {
                    *i < self.cursor_x || *i > start_pos
                }
            })
            .map(|(_, c)| c)
            .collect()
    }

    pub fn rsh_move_cursor(&mut self, prompt: Prompt) {
        let mut stdout = stdout();
        let mut range_string = String::new();
        let start_pos = self.cursor_x;
        // 範囲選択がどの方向に進んでいるか
        let mut direction: &str = "left";

        // 初期値
        if self.now_mode == Mode::Nomal {
            self.cursor_x = self.buffer.buffer.len();
            self.char_count = self.buffer.buffer.chars().count();
        }

        enable_raw_mode().unwrap();
        loop {
            if start_pos > self.cursor_x {
                direction = "left";
            } else if start_pos <= self.cursor_x {
                direction = "right";
            }
            // デザイン部分
            if self.now_mode == Mode::Visual {
                //選択されている部分
                if direction == "left" {
                    // self.cursor_x..start_pos => 選択している範囲
                    for pos in self.cursor_x..self.buffer.buffer.len() {
                        execute!(
                            stdout,
                            MoveToColumn((prompt.len() + pos) as u16),
                            if pos <= start_pos {
                                SetBackgroundColor(Color::Blue)
                            } else {
                                SetBackgroundColor(Color::Reset)
                            },
                            Print(self.buffer.buffer.chars().nth(pos).unwrap()),
                        )
                        .unwrap();
                    }
                } else {
                    for pos in start_pos..self.buffer.buffer.len() {
                        execute!(
                            stdout,
                            MoveToColumn((prompt.len() + pos) as u16),
                            if pos <= self.cursor_x {
                                SetBackgroundColor(Color::Blue)
                            } else {
                                SetBackgroundColor(Color::Reset)
                            },
                            Print(self.buffer.buffer.chars().nth(pos).unwrap()),
                        )
                        .unwrap();
                    }
                }
                // 選択されていない部分
                execute!(
                    stdout,
                    SetBackgroundColor(Color::Reset),
                    MoveToColumn((prompt.len() + self.cursor_x) as u16)
                )
                .unwrap();
            }

            if let Event::Key(KeyEvent {
                code,
                modifiers: _,
                kind: _,
                state: _,
            }) = read().unwrap()
            {
                // "( )" ← この文字があると不具合が発生する
                match code {
                    KeyCode::Esc => {
                        self.set_mode(Mode::Nomal);
                        break;
                    }
                    KeyCode::Char('h') => {
                        // 相対移動
                        // Bufferの文字列内でカーソルを移動させるため
                        if self.cursor_x > 0 {
                            execute!(stdout, MoveLeft(1)).unwrap();
                            stdout.flush().unwrap();
                            if direction == "right" {
                                range_string.pop();
                            } else {
                                range_string.push(
                                    self.buffer.buffer.chars().nth(self.cursor_x - 1).unwrap(),
                                );
                            }
                            self.cursor_x -= 1;
                        }
                    }
                    KeyCode::Char('l') => {
                        // 相対移動
                        // Bufferの文字列内でカーソルを移動させるため
                        if self.cursor_x < self.buffer.buffer.len() {
                            execute!(stdout, MoveRight(1)).unwrap();
                            stdout.flush().unwrap();
                            if direction == "left" {
                                range_string.pop();
                            } else {
                                range_string
                                    .push(self.buffer.buffer.chars().nth(self.cursor_x).unwrap());
                            }
                            self.cursor_x += 1;
                        }
                    }
                    KeyCode::Char('i') => {
                        self.now_mode = Mode::Input;
                        break;
                    }
                    KeyCode::Char('v') => {
                        self.now_mode = Mode::Visual;
                        break;
                    }
                    KeyCode::Char('d') => {
                        // 選択された文字列を削除
                        if direction == "left" {
                            range_string = range_string.chars().rev().collect();
                        }
                        for _ in 0..range_string.len() {
                            execute!(stdout, MoveLeft(1)).unwrap();
                            execute!(stdout, Print(" ")).unwrap();
                            execute!(stdout, MoveLeft(1)).unwrap();
                        }
                        self.buffer.buffer = self.get_string_at_cursor(start_pos);
                        self.cursor_x = self.buffer.buffer.len();
                        self.char_count = self.buffer.buffer.chars().count();
                        self.now_mode = Mode::Nomal;
                        break;
                    }
                    _ => {}
                }
                std::io::stdout().flush().unwrap();
            }
        }
        disable_raw_mode().unwrap();
        /*
                if direction == "left" {
                    range_string = range_string.chars().rev().collect();
                }
                println!("\n{}", range_string);
        */
        stdout.flush().unwrap();
    }

    pub fn rsh_loop(&mut self) -> Result<Status, RshError> {
        let mut stdout = stdout();

        self.ignore_tty_signals();

        execute!(stdout, Print("\n"),)
            .map_err(|_| RshError::new("Failed to print directory"))
            .unwrap();

        // 絶対値なので相対移動になるようになんとかする
        let _ = execute!(stdout, MoveTo(0, 0), Clear(ClearType::All));

        self.cursor_x = self.buffer.buffer.len();
        self.char_count = self.buffer.buffer.chars().count();

        let mut isnt_ascii_counter = 0;

        loop {
            enable_raw_mode().unwrap();

            let _ = self.set_prompt();
            let prompt = Prompt::new(
                username(),
                self.get_current_dir_as_vec(),
                self.return_code,
                self.now_mode,
            );

            self.rsh_print(self.buffer.buffer.clone());

            match self.now_mode {
                Mode::Nomal => {
                    self.rsh_move_cursor(prompt);
                }
                Mode::Input => {
                    // カーソルを指定の位置にずらす(Nomalモードで移動があった場合表示はここで更新される)
                    execute!(stdout, MoveToColumn((prompt.len() + self.cursor_x) as u16)).unwrap();

                    // 入力を取得
                    let mut pushed_tab = false;
                    let mut stack_buffer = String::new();
                    let mut tab_counter = 0;

                    enable_raw_mode().unwrap();

                    loop {
                        self.get_directory_contents("./");
                        // カーソルを指定の位置にずらす(Nomalモードで移動があった場合表示はここで更新される)
                        execute!(
                            stdout,
                            MoveToColumn(
                                (prompt.len() + self.cursor_x - isnt_ascii_counter) as u16
                            )
                        )
                        .unwrap();

                        // キー入力の取得
                        if let Event::Key(KeyEvent {
                            code,
                            modifiers: _,
                            kind: _,
                            state: _,
                        }) = read().unwrap()
                        {
                            match code {
                                KeyCode::Esc => {
                                    self.now_mode = Mode::Nomal;
                                    break;
                                }
                                KeyCode::Tab => {
                                    if !pushed_tab {
                                        // 現時点で入力されている文字のバックアップ
                                        stack_buffer = self.buffer.buffer.clone();
                                    }
                                    // コマンドDBの取得
                                    self.get_executable_commands();
                                    self.get_directory_contents("./");
                                    self.get_rshhistory_contents().unwrap();

                                    // 予測されるコマンドを取得
                                    if let Ok(autocomplete) =
                                        self.rsh_char_search(stack_buffer.clone(), &mut tab_counter)
                                    {
                                        self.buffer.buffer = autocomplete;
                                    }

                                    self.cursor_x = self.buffer.buffer.len();
                                    self.char_count = self.buffer.buffer.chars().count();

                                    pushed_tab = true;
                                    tab_counter += 1;
                                }
                                KeyCode::Enter => break,
                                KeyCode::Char(' ') => {
                                    // TABの直後にSpaceが入力された場合
                                    self.buffer.buffer.insert(self.cursor_x, ' ');
                                    pushed_tab = false;
                                    self.cursor_x += 1;
                                    self.char_count += 1;
                                }
                                _ => {
                                    self.buffer.buffer = match code {
                                        KeyCode::Backspace => {
                                            println!(
                                                "\n{:?}, cursor_x: {}, char_count{}",
                                                self.buffer.buffer, self.cursor_x, self.char_count
                                            );
                                            // カーソルがバッファの範囲内にある場合
                                            if self.char_count <= self.buffer.buffer.len()
                                                && self.cursor_x > 0
                                            {
                                                // 要素を削除
                                                if self
                                                    .buffer
                                                    .buffer
                                                    .is_char_boundary(self.cursor_x - 1)
                                                {
                                                    self.buffer.buffer.remove(self.cursor_x - 1);
                                                } else {
                                                    // それ以外
                                                    let mut buffer_graphemes = self
                                                        .buffer
                                                        .buffer
                                                        .graphemes(true)
                                                        .collect::<Vec<&str>>();
                                                    buffer_graphemes.remove(self.char_count - 1);
                                                    stdout.flush().unwrap();
                                                    self.buffer.buffer = buffer_graphemes.concat();
                                                    isnt_ascii_counter -= 1;
                                                    self.cursor_x -= 2;
                                                }
                                                // cursor_xはマルチバイト文字がある場合マルチバイト文字の数 *3 + 普通の文字数 = char_countになる
                                                // git commit -m "fix: 日本語 まで入力して削除しようとすると計算が合わなくなる
                                                // char_count と　cursor_xの釣り合いが取れない
                                                // cursor_xがきちんとマイナスされていない？
                                                // char_countがきちんとプラスされていない？
                                                self.cursor_x -= 1;
                                                self.char_count -= 1;
                                            }
                                            self.buffer.buffer.clone()
                                        }
                                        KeyCode::Char(c) => {
                                            self.char_count += 1;
                                            if c.is_ascii() {
                                                self.buffer.buffer.insert(self.cursor_x, c);
                                                self.cursor_x += 1;
                                            } else {
                                                let mut buf = [0; 4];
                                                let c_str = c.encode_utf8(&mut buf);
                                                for ch in c_str.chars() {
                                                    self.buffer.buffer.insert(self.cursor_x, ch);
                                                    self.cursor_x += c_str.len();
                                                    // 全角文字の場合は文字のとる幅から余分な文を減らすためカウンタを増やす
                                                    isnt_ascii_counter += 1;
                                                }
                                            }
                                            self.buffer.buffer.clone()
                                        }
                                        _ => self.buffer.buffer.clone(),
                                    };
                                }
                            }
                        }

                        // コマンド実行履歴の中からbufferで始まるものを取得
                        let history_matches: Vec<String> = self
                            .history_database
                            .iter()
                            .filter(|history| {
                                history.get_command().starts_with(&self.buffer.buffer)
                            })
                            .map(|history| history.get_command().to_string())
                            .collect();

                        // 利用可能なコマンドの中からbufferで始まるものを取得
                        let matches = self
                            .command_database
                            .iter()
                            .filter(|command| command.starts_with(&self.buffer.buffer));

                        // 上記を配列に変換
                        let mut filtered_commands: Vec<String> =
                            history_matches.into_iter().map(|s| s.to_string()).collect();
                        filtered_commands.extend(matches.map(|s| s.to_string()));

                        // もしもコマンドが見つからなかった場合、環境変数を利用して参照しなおす
                        if filtered_commands.len() == 0 {
                            for env_path in self.env_database.clone() {
                                // command_databaseの中からenv_path/bufferで始まるものを取得
                                let matches = self.command_database.iter().filter(|command| {
                                    let command_path =
                                        format!("{}/{}", env_path, self.buffer.buffer);
                                    command.starts_with(&self.buffer.buffer)
                                        || command_path.starts_with(&self.buffer.buffer)
                                });
                                // 上記を配列に変換
                                filtered_commands = matches.map(|s| s.to_string()).collect();
                            }
                        }

                        let _ = self.set_prompt();

                        let print_buf_parts: Vec<String> =
                            self.rsh_split_line(self.buffer.buffer.clone()); //print_buf.split_whitespace().collect();

                        // 瓶覗 かめのぞき
                        // コマンドの色
                        self.set_prompt_color("#457E7D".to_string()).unwrap();
                        // コマンド・コマンド引数ともに表示
                        for (i, part) in print_buf_parts.iter().enumerate() {
                            // 一つのコマンド
                            execute!(stdout, Print(part)).unwrap();

                            if i < print_buf_parts.len() - 1 {
                                execute!(stdout, Print(" ")).unwrap();
                                self.set_prompt_color("#AC6383".to_string()).unwrap();
                            }
                        }

                        // 補完されるコマンドがある場合描写する
                        if filtered_commands.len() > 0 {
                            // 部分的に一致しているコマンドの先頭の要素からbufferから先を取得
                            let print_buf_suffix = self.rsh_split_line(
                                filtered_commands[0][self.buffer.buffer.len()..].to_string(),
                            );

                            // コマンド補完表示の色
                            self.set_prompt_color("#938274".to_string()).unwrap();

                            let mut print_length = 0;
                            // コマンド・コマンド引数ともに表示
                            for (i, part) in print_buf_suffix.iter().enumerate() {
                                execute!(stdout, Print(part)).unwrap();
                                print_length += part.len();
                                if i < print_buf_suffix.len() - 1 {
                                    execute!(stdout, Print(" ")).unwrap();
                                    print_length += 1;
                                }
                            }

                            if print_length > 0 {
                                execute!(stdout, MoveLeft(print_length as u16),).unwrap();
                            }
                        }
                    }

                    disable_raw_mode().unwrap();

                    self.cursor_x = 0;
                    self.char_count = 0;
                    self.set_prompt_color("#ECE1B4".to_string())?;
                    execute!(stdout, MoveToColumn(0)).unwrap();

                    if self.now_mode != Mode::Input {
                        continue;
                    }
                    execute!(stdout, Print("\n")).unwrap();

                    // 入力を実行可能な形式に分割
                    let args = self.rsh_split_line(self.buffer.buffer.clone());

                    // 実行可能なコマンド一覧を取得
                    self.get_executable_commands();

                    // 履歴ファイルが存在するか？
                    if let Err(err) = self.get_rshhistory_contents() {
                        self.eprintln(&format!("Error: {}", err.message));
                    }
                    // 環境変数ファイルが存在するか？
                    if let Err(_) = self.get_rshenv_contents() {
                        //self.eprintln(&format!("Error: {}", err.message));
                        self.exists_rshenv = false;
                    }

                    self.buffer.buffer = String::new();
                    // 分割したコマンドを実行
                    match self.rsh_execute(args) {
                        Ok(status) => match status {
                            Status::Success => continue,
                            exit @ Status::Exit => return Ok(exit),
                        },
                        err @ Err(_) => return err,
                    };
                }

                Mode::Visual => {
                    self.rsh_move_cursor(prompt);
                    if self.now_mode != Mode::Visual {
                        continue;
                    }
                }
                _ => {}
            }
        }
    }

    pub fn new() -> Self {
        Self {
            prompt: String::new(),
            buffer: Buffer::new(),
            env_database: Vec::new(),
            history_database: Vec::new(),
            command_database: Vec::new(),
            return_code: 0,
            exists_rshenv: false,
            now_mode: Mode::Nomal,
            cursor_x: 0,
            char_count: 0,
        }
    }
}

fn main() {
    let mut rsh = Rsh::new();
    let code = rsh.rsh_loop();
    match code {
        Err(err) => {
            if let Err(e) = execute!(
                stdout(),
                MoveToColumn(0),
                Clear(ClearType::UntilNewLine),
                SetForegroundColor(Color::White),
                Print("rsh: "),
                SetForegroundColor(Color::Red),
                Print(err.message),
                Print("\n"),
                SetForegroundColor(Color::White),
            ) {
                eprintln!("Failed to execute command: {}", e);
            }
        }
        _ => (),
    }
}
