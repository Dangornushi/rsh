use crate::error::error::{RshError, Status};
use crate::evaluator;
use crate::log::log_maneger::csv_reader;
use crate::log::log_maneger::csv_writer;
use crate::log::log_maneger::History;
use crate::parser::parse::Parse;
use nix::sys::signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal};

use colored::Colorize;
use crossterm::{
    cursor::{MoveLeft, MoveRight, MoveTo, MoveToColumn, SetCursorStyle},
    event::{poll, read, Event, KeyCode, KeyEvent},
    execute,
    style::{Color, Print, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use std::time::Duration;
use std::{
    env, fs,
    io::{stdout, Write},
};
use unicode_segmentation::UnicodeSegmentation;
use whoami::username;

#[derive(PartialEq, Clone)]
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

#[derive(PartialEq, Clone, Copy, Debug)]
enum Mode {
    Nomal,
    Visual,
    Input,
}

#[derive(PartialEq, Clone)]
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

#[derive(PartialEq, Clone)]
pub struct Rsh {
    prompt: Prompt,
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
    pub fn open_profile(&self, path: &str) -> Result<String, RshError> {
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

    pub fn eprintln(&self, message: &str) {
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

    pub fn get_history_database(&self) -> Vec<History> {
        self.history_database.clone()
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

    fn rsh_get_command_database(&self, search_string: String) -> Vec<String> {
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

        filtered_commands
    }

    fn rsh_char_search(
        &self,
        search_string: String,
        counter: &mut usize,
    ) -> Result<String, RshError> {
        let filtered_commands = self.rsh_get_command_database(search_string.clone());

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
                if start_pos < self.char_count {
                    *i < start_pos || *i > self.char_count
                } else {
                    *i < self.char_count || *i > start_pos
                }
            })
            .map(|(_, c)| c)
            .collect()
    }

    fn initializations_cursor_view(&mut self, stdout: &mut std::io::Stdout) {
        // カーソルを行の最後尾に移動
        let mut count = 0;
        for (i, c) in self.buffer.buffer.chars().enumerate() {
            if i >= self.char_count {
                break;
            }
            count += 1;
            if !c.is_ascii() {
                count += 1;
            }
        }
        if let Err(e) = execute!(stdout, MoveToColumn((self.prompt.len() + count) as u16)) {
            self.eprintln(&format!("Failed to move cursor: {}", e));
        }
    }

    pub fn move_cursor_left(
        &mut self,
        stdout: &mut std::io::Stdout,
        direction: &str,
        range_string: &mut String,
    ) {
        // 相対移動
        // Bufferの文字列内でカーソルを移動させるため
        let char_len = self
            .buffer
            .buffer
            .chars()
            .nth(self.char_count - 1)
            .unwrap()
            .len_utf8()
            - 1;
        if direction == "right" {
            // 今までl押下で右側にカーソルを動かしていたが、今はhをおしている
            // start_posまで戻った際はdirectionをleftに変更する
            range_string.pop();
            /*
            if range_string.len() == 0 {
                direction = "left";
            } else {
                for pos in start_pos..self.char_count {
                    execute!(
                        stdout,
                        MoveToColumn((self.prompt.len() + pos) as u16),
                        SetBackgroundColor(Color::Reset),
                        Print(self.buffer.buffer.chars().nth(pos).unwrap()),
                    )
                    .unwrap();
                }
                //      execute!(stdout, MoveLeft(char_len as u16),).unwrap();
            }*/
        }
        if direction == "left" {
            // h押下で左側にカーソルを動かしている
            range_string.push(self.buffer.buffer.chars().nth(self.char_count - 1).unwrap());
            /*
            if self.now_mode == Mode::Visual {
                for pos in self.char_count - 1..start_pos + 1 {
                    if start_pos - 1 < pos {
                        execute!(
                            stdout,
                            MoveToColumn((self.prompt.len() + pos) as u16),
                            SetBackgroundColor(Color::Reset),
                        )
                        .unwrap();
                    } else {
                        execute!(
                            stdout,
                            MoveToColumn((self.prompt.len() + pos) as u16),
                            SetBackgroundColor(Color::Blue),
                            Print(self.buffer.buffer.chars().nth(pos).unwrap())
                        )
                        .unwrap();
                    }
                }
            }*/
            execute!(stdout, MoveLeft(char_len as u16)).unwrap();
        }
        self.cursor_x -= char_len + 1;
        self.char_count -= 1;
    }

    pub fn move_cursor_right(
        &mut self,
        stdout: &mut std::io::Stdout,
        direction: &str,
        range_string: &mut String,
    ) {
        // 相対移動
        // Bufferの文字列内でカーソルを移動させるため
        let char_len = self
            .buffer
            .buffer
            .chars()
            .nth(self.char_count)
            .unwrap()
            .len_utf8();

        if self.now_mode == Mode::Visual {
            if direction == "left" {
                // l押下でカーソルを右側に動かしている
                /*
                if self.now_mode == Mode::Visual {

                    execute!(
                        stdout,
                        MoveToColumn((self.prompt.len() + self.char_count) as u16),
                        SetBackgroundColor(Color::Reset),
                        Print(
                            self.buffer
                                .buffer
                                .chars()
                                .nth(self.char_count)
                                .unwrap()
                        ),
                    )
                    .unwrap();
                    for pos in self.char_count + 1..start_pos {
                        execute!(
                            stdout,
                            MoveToColumn((self.prompt.len() + pos) as u16),
                            SetBackgroundColor(Color::Blue),
                            Print(self.buffer.buffer.chars().nth(pos).unwrap()),
                        )
                        .unwrap();
                    }
                }
                */
                range_string.pop();
            }
            if direction == "right" {
                // 今までh押下で左側にカーソルを動かしていたが、今はlをおしている
                /*
                if self.now_mode == Mode::Visual {
                    for pos in start_pos..self.buffer.buffer.chars().count() {
                        if pos < self.char_count {
                            execute!(
                                stdout,
                                MoveToColumn((self.prompt.len() + pos) as u16),
                                SetBackgroundColor(Color::Blue),
                            )
                            .unwrap();
                        } else {
                            /**/
                            execute!(
                                stdout,
                                MoveToColumn((self.prompt.len() + pos) as u16),
                                SetBackgroundColor(Color::Reset),
                            )
                            .unwrap();
                        }
                    }
                }
                */
                range_string.push(self.buffer.buffer.chars().nth(self.char_count).unwrap());
            }
        }

        self.cursor_x += char_len;
        self.char_count += 1;
        execute!(stdout, MoveRight(char_len as u16)).unwrap();
    }

    pub fn rsh_move_cursor(&mut self) {
        let mut stdout = stdout();
        let mut range_string = String::new();
        let start_pos = self.char_count;
        //let start_cursor_x = self.cursor_x;
        // 範囲選択がどの方向に進んでいるか
        let mut direction: &str = "";
        let mut direction_set = false;

        // 初期値
        if self.now_mode == Mode::Nomal {
            //self.initializations_cursor_value();
        }

        //enable_raw_mode().unwrap();

        loop {
            if !direction_set {
                if start_pos > self.char_count {
                    direction_set = true;
                    direction = "left";
                } else if start_pos < self.char_count {
                    direction_set = true;
                    direction = "right";
                }
            }
            self.initializations_cursor_view(&mut stdout);
            // デザイン部分

            // キー入力の取得
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
                        if self.char_count > 0 {
                            self.move_cursor_left(&mut stdout, direction, &mut range_string);
                        }
                    }
                    KeyCode::Char('l') => {
                        if self.char_count + 1 < self.buffer.buffer.chars().count() {
                            self.move_cursor_right(&mut stdout, direction, &mut range_string);
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

                        /*
                        if direction == "left" {
                            range_string = range_string.chars().rev().collect();
                        }
                        for _ in 0..range_string.len() {
                            execute!(stdout, MoveLeft(1)).unwrap();
                            execute!(stdout, Print(" ")).unwrap();
                            execute!(stdout, MoveLeft(1)).unwrap();
                        }*/
                        self.buffer.buffer = self.get_string_at_cursor(start_pos);
                        self.cursor_x = self.buffer.buffer.len();
                        self.char_count = self.buffer.buffer.chars().count();
                        self.now_mode = Mode::Nomal;
                        break;
                    }
                    KeyCode::Char('a') => {
                        if self.char_count < self.buffer.buffer.chars().count() {
                            self.move_cursor_right(&mut stdout, direction, &mut range_string);
                        }
                        self.now_mode = Mode::Input;
                        break;
                    }

                    _ => {}
                }
                std::io::stdout().flush().unwrap();
            }
        }
        //disable_raw_mode().unwrap();
        stdout.flush().unwrap();
    }

    fn ignore_tty_signals(&self) {
        let sa = SigAction::new(SigHandler::SigIgn, SaFlags::empty(), SigSet::empty());
        unsafe {
            sigaction(Signal::SIGTSTP, &sa).unwrap();
            sigaction(Signal::SIGTTIN, &sa).unwrap();
            sigaction(Signal::SIGTTOU, &sa).unwrap();
        }
    }

    fn get_filterd_commands(&self, buffer: String) -> Vec<String> {
        // コマンド実行履歴の中からbufferで始まるものを取得
        let mut history_matches: Vec<String> = self
            .history_database
            .iter()
            .filter(|history| history.get_command().starts_with(&buffer))
            .map(|history| history.get_command().to_string())
            .collect();

        history_matches.reverse();

        // 利用可能なコマンドの中からbufferで始まるものを取得
        let matches = self
            .command_database
            .iter()
            .filter(|command| command.starts_with(&buffer));

        // 上記を配列に変換
        let mut filtered_commands: Vec<String> =
            history_matches.into_iter().map(|s| s.to_string()).collect();
        filtered_commands.extend(matches.map(|s| s.to_string()));
        filtered_commands
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

        loop {
            let _ = self.set_prompt();
            let prompt = Prompt::new(
                username(),
                self.get_current_dir_as_vec(),
                self.return_code,
                self.now_mode,
            );

            self.prompt = prompt;

            self.rsh_print(self.buffer.buffer.clone());

            match self.now_mode {
                Mode::Nomal => {
                    execute!(stdout, SetCursorStyle::DefaultUserShape).unwrap();
                    self.rsh_move_cursor();
                }
                Mode::Input => {
                    // Input モードの初期化
                    // 実行可能なコマンド一覧を取得
                    self.get_executable_commands();

                    // 履歴ファイルが存在するか？
                    if let Err(err) = self.get_rshhistory_contents() {
                        self.eprintln(&format!("Error: {}", err.message));
                    }
                    // 環境変数ファイルが存在するか？
                    if let Err(_) = self.get_rshenv_contents() {
                        self.exists_rshenv = false;
                    }

                    execute!(stdout, SetCursorStyle::SteadyBar).unwrap();

                    // 入力を取得
                    let mut pushed_tab = false;
                    let mut stack_buffer = String::new();
                    let mut tab_counter = 0;

                    self.get_directory_contents("./");
                    self.initializations_cursor_view(&mut stdout);
                    // 文字が入力ごとにループが回る
                    let mut history_index = self.history_database.len();
                    let mut history_buf = String::new();
                    let mut has_referenced_history = false;

                    loop {
                        // 文字が入力ごとにループが回る
                        // カーソルを指定の位置にずらす
                        execute!(
                            stdout,
                            MoveToColumn((self.prompt.len() + self.cursor_x) as u16)
                        )
                        .unwrap();

                        // キー入力の取得
                        if poll(Duration::from_millis(5))
                            .map_err(|_| RshError::new("Failed to poll"))?
                        {
                            if let Ok(Event::Key(KeyEvent {
                                code,
                                modifiers: _,
                                kind: _,
                                state: _,
                            })) = read()
                            {
                                match code {
                                    KeyCode::Up => {
                                        // 初めて履歴を参照した時のみ打ち込まれていた文字を保存
                                        if !has_referenced_history {
                                            history_buf = self.buffer.buffer.clone();
                                        }
                                        if 0 < history_index {
                                            // 履歴の中から一つ前のコマンドを取得
                                            history_index -= 1;
                                            self.buffer.buffer = self
                                                .history_database
                                                .get(history_index)
                                                .unwrap()
                                                .get_command()
                                                .to_string();
                                            self.cursor_x = self.buffer.buffer.len();
                                            self.char_count = self.buffer.buffer.chars().count();
                                            has_referenced_history = true;
                                        }
                                    }
                                    KeyCode::Down => {
                                        // 履歴の中から一つ前のコマンドを取得
                                        //  自分が履歴を見るまでターミナルに打ち込んでいた文字を反映
                                        if history_index + 1 == self.history_database.len() {
                                            self.buffer.buffer = history_buf.clone();

                                            self.cursor_x = self.buffer.buffer.len();
                                            self.char_count = self.buffer.buffer.chars().count();
                                            has_referenced_history = false;
                                        }
                                        if 1 < history_index
                                            && history_index < self.history_database.len() - 1
                                        {
                                            history_index += 1;
                                            self.buffer.buffer = self
                                                .history_database
                                                .get(history_index)
                                                .unwrap()
                                                .get_command()
                                                .to_string();
                                            self.cursor_x = self.buffer.buffer.len();
                                            self.char_count = self.buffer.buffer.chars().count();
                                        }
                                    }
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
                                        if let Ok(autocomplete) = self
                                            .rsh_char_search(stack_buffer.clone(), &mut tab_counter)
                                        {
                                            self.buffer.buffer = autocomplete;
                                        }

                                        self.cursor_x = self.buffer.buffer.len();
                                        self.char_count = self.buffer.buffer.chars().count();

                                        pushed_tab = true;
                                        tab_counter += 1;
                                    }
                                    KeyCode::Enter => {
                                        self.cursor_x = 0;
                                        self.char_count = 0;
                                        break;
                                    }
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
                                                        if self
                                                            .buffer
                                                            .buffer
                                                            .chars()
                                                            .nth(self.cursor_x - 1)
                                                            == Some(' ')
                                                        {
                                                        }
                                                        self.buffer
                                                            .buffer
                                                            .remove(self.cursor_x - 1);
                                                    } else {
                                                        // それ以外
                                                        let mut buffer_graphemes = self
                                                            .buffer
                                                            .buffer
                                                            .graphemes(true)
                                                            .collect::<Vec<&str>>();

                                                        if buffer_graphemes.get(self.char_count - 1)
                                                            == Some(&" ")
                                                        {
                                                        }

                                                        buffer_graphemes
                                                            .remove(self.char_count - 1);
                                                        self.buffer.buffer =
                                                            buffer_graphemes.concat();
                                                        //isnt_ascii_counter -= 1;
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
                                                        self.buffer
                                                            .buffer
                                                            .insert(self.cursor_x, ch);
                                                        self.cursor_x += c_str.len();
                                                    }
                                                }
                                                self.buffer.buffer.clone()
                                            }
                                            _ => self.buffer.buffer.clone(),
                                        };
                                    }
                                }
                            }
                        } else {
                            continue;
                        }
                        let mut filtered_commands =
                            self.get_filterd_commands(self.buffer.buffer.clone());
                        // もしもコマンドが見つからなかった場合、環境変数を利用して参照しなおす
                        if filtered_commands.len() == 0 {
                            // 環境変数を一つずつ取得
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
                                // 　ここのインデックスを上下キーで変更する
                                //                                filtered_commands[filtered_commands.len() - 1]
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

                    // Inputモードから離脱
                    if self.now_mode != Mode::Input {
                        if self.char_count > 0 {
                            self.move_cursor_left(&mut stdout, "left", &mut String::new());
                        }
                        continue;
                    }

                    //disable_raw_mode().unwrap();
                    //self.cursor_x = 0;
                    //self.char_count = 0;
                    self.set_prompt_color("#ECE1B4".to_string())?;
                    execute!(stdout, MoveToColumn(0), Print("\n")).unwrap();

                    disable_raw_mode().unwrap();
                    // コマンドの実行
                    let mut buffer = &mut self.buffer.buffer.clone();

                    self.execute_commands(&mut buffer);
                    self.buffer.buffer.clear();
                    enable_raw_mode().unwrap();
                }
                Mode::Visual => {
                    execute!(stdout, SetCursorStyle::BlinkingUnderScore).unwrap();
                    self.rsh_move_cursor();
                    if self.now_mode != Mode::Visual {
                        continue;
                    }
                }
            }
        }
    }

    pub fn execute_commands(&mut self, command: &mut String) -> i32 {
        // CSV
        let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let _ = self
            .open_profile(".rsh_history")
            .map(|path| csv_writer(command.clone(), time, &path));
        // ---
        // 入力を実行可能な形式に分割
        let parsed = Parse::parse_node(&command).clone();

        // ASTの評価
        if let Ok((_, node)) = parsed {
            // 分割したコマンドを実行
            let code = evaluator::evaluator::Evaluator::new(self.to_owned()).evaluate(node);
            *command = String::new();
            code
        } else {
            *command = String::new();
            self.eprintln(&format!("Failed to parse input"));
            1
        }
    }

    pub fn new() -> Self {
        Self {
            prompt: Prompt::new(username(), vec!["".to_string()], 0, Mode::Nomal),
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

impl Drop for Rsh {
    fn drop(&mut self) {
        // 必要なクリーンアップをここで実行
        // drop以外の名前を定義することはできない
        execute!(
            stdout(),
            SetForegroundColor(Color::White),
            SetCursorStyle::DefaultUserShape
        )
        .unwrap();
    }
}
