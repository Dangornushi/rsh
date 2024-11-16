mod command;
mod error;

use colored::Colorize;
use crossterm::{
    cursor,
    event::{Event, KeyCode},
    execute,
    style::{Color, Print, SetForegroundColor},
    terminal,
};
use error::error::{RshError, Status};
use nix::sys::wait::*;
use nix::{
    errno::Errno,
    sys::{
        signal::{sigaction, SaFlags, SigAction, SigHandler, SigSet, Signal},
        wait::waitpid,
    },
    unistd::{close, execvp, fork, getpgrp, pipe, read, setpgid, tcsetpgrp, ForkResult},
};
use std::ffi::CString;
use std::io::{stdin, stdout, Read, Write};
use std::thread;
use std::time::Duration;
use whoami::username;

fn rsh_read_line() -> String {
    let mut buffer = String::new();
    let mut stdin = std::io::stdin();

    std::io::stdout().flush().unwrap();

    loop {
        let mut b = [0; 1];
        match stdin.read(&mut b) {
            Ok(n) if n == 1 => {
                let c = b[0] as char;
                if c == '\n' {
                    break;
                } else {
                    buffer.push(b[0] as char);
                }
            }
            Ok(_) => {
                println!("invalid input");
            }
            Err(e) => {
                eprintln!("エラーが発生しました: {}", e);
                break;
            }
        }
    }
    buffer.trim().to_string()
}

fn rsh_split_line(line: String) -> Vec<String> {
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

fn ignore_tty_signals() {
    let sa = SigAction::new(SigHandler::SigIgn, SaFlags::empty(), SigSet::empty());
    unsafe {
        sigaction(Signal::SIGTSTP, &sa).unwrap();
        sigaction(Signal::SIGTTIN, &sa).unwrap();
        sigaction(Signal::SIGTTOU, &sa).unwrap();
    }
}

fn restore_tty_signals() {
    let sa = SigAction::new(SigHandler::SigDfl, SaFlags::empty(), SigSet::empty());
    unsafe {
        sigaction(Signal::SIGTSTP, &sa).unwrap();
        sigaction(Signal::SIGTTIN, &sa).unwrap();
        sigaction(Signal::SIGTTOU, &sa).unwrap();
    }
}

fn rsh_launch(args: Vec<String>) -> Result<Status, RshError> {
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
            restore_tty_signals();

            close(pipe_write).unwrap();

            loop {
                let mut buf = [0];
                match read(pipe_read, &mut buf) {
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
                .map_err(|_| RshError::new("Child Process failed"))

            // -------------
        }
    }
}

fn rsh_cursor_test() -> Result<(), std::io::Error> {
    let mut stdin = stdin();
    let mut buffer = [0];
    let mut rgb = 0;

    terminal::enable_raw_mode()?;

    // 文字の出力
    execute!(stdout(), Print("Hello, world!"))?;

    loop {
        thread::sleep(Duration::from_millis(1));
        // カーソルを先頭に移動し、文字を消去
        execute!(stdout(), cursor::MoveToColumn(1), cursor::MoveToNextLine(1))?;

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
    }
    // 元の状態に戻す
    terminal::disable_raw_mode()?;

    Ok(())
}

fn rsh_execute(args: Vec<String>) -> Result<Status, RshError> {
    if let Option::Some(arg) = args.get(0) {
        return match arg.as_str() {
            // cd: ディレクトリ移動の組み込みコマンド
            "cd" => command::cd::rsh_cd(if let Option::Some(dir) = args.get(1) {
                dir
            } else {
                ""
            }),
            // ロゴ表示
            "%logo" => command::logo::rsh_logo(),
            "%" => {
                let _ = rsh_cursor_test();
                Ok(Status::Success)
            }
            // exit: 終了用の組み込みコマンド
            "exit" => command::exit::rsh_exit(),
            // none: 何もなければコマンド実行
            _ => rsh_launch(args),
        };
    }
    Ok(Status::Success)
}

fn get_current_dir_as_vec() -> Vec<String> {
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

fn rhs_loop() -> Result<Status, RshError> {
    let cursor = ">";

    ignore_tty_signals();

    loop {
        // ui ------------------------------------------------------------------
        print!("{}: ", username().green().bold());

        // 文字色処理アルゴリズム ---------------------------------
        let dir_s = get_current_dir_as_vec();
        for i in dir_s {
            print!("{}/", i.white().bold()); //.custom_color(path_base_color));
        }
        // --------------------------------------------------------
        print!(" {} ", cursor);
        // ---------------------------------------------------------------------

        let line = rsh_read_line();
        let args = rsh_split_line(line);

        match rsh_execute(args) {
            Ok(status) => match status {
                Status::Success => continue,
                exit @ Status::Exit => return Ok(exit),
            },
            err @ Err(_) => return err,
        };
    }
}

fn main() {
    let code = rhs_loop();
    println!("> {:?}", code);
}
