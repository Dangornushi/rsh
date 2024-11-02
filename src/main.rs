use colored::Colorize;
use colored::CustomColor;
use nix::sys::wait::*;
use nix::unistd::*;
use std::env::current_dir;
use std::ffi::CString;
use std::io::Read;
use std::io::Write;
use whoami::username;

#[derive(Debug)]
pub struct RshError {
    pub message: String,
}

impl RshError {
    pub fn new(message: &str) -> RshError {
        RshError {
            message: message.to_string(),
        }
    }
}

#[derive(Debug)]
pub enum Status {
    Success,
    Exit,
}

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
                println!("予期しない入力");
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
    line.split(" ").map(String::from).collect()
}

fn rsh_cd() -> Result<Status, RshError> {
    println!("Rsh cd");
    Ok(Status::Success)
}

fn rsh_exit() -> Result<Status, RshError> {
    Ok(Status::Exit)
}

fn rsh_launch(args: Vec<String>) -> Result<Status, RshError> {
    let pid = fork().map_err(|_| RshError::new("fork failed"))?;

    match pid {
        ForkResult::Parent { child } => {
            let wait_pid_result =
                waitpid(child, None).map_err(|err| RshError::new(&format!("{}", err)));
            match wait_pid_result {
                Ok(WaitStatus::Exited(_, return_code)) => {
                    println!("Exited: {}", return_code);
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
            let path = CString::new(args[0].to_string()).unwrap();
            let args = if args.len() > 1 {
                CString::new(args[1].to_string()).unwrap()
            } else {
                CString::new("").unwrap()
            };

            execvp(&path, &[path.clone(), args])
                .map(|_| Status::Success)
                .map_err(|_| RshError::new("Child Process failed"))
        }
    }
}

fn rsh_execute(args: Vec<String>) -> Result<Status, RshError> {
    if let Option::Some(arg) = args.get(0) {
        return match arg.as_str() {
            // cd: ディレクトリ移動の組み込みコマンド
            "cd" => rsh_cd(),
            // exit: 終了用の組み込みコマンド
            "exit" => rsh_exit(),
            // none: 何もなければコマンド実行
            _ => rsh_launch(args),
        };
    }
    Ok(Status::Success)
}

fn get_current_dir_as_vec() -> Vec<String> {
    let current_dir = std::env::current_dir().unwrap();
    let path = current_dir.as_path();
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect()
}

fn rhs_loop() -> Result<Status, RshError> {
    let cursor = ">";
    let mut path_base_color = CustomColor::new(200, 0, 0);
    let r = 135;
    let g = 28;
    let b = 267;

    let mut inc_r = 0;
    let mut inc_g = 0;
    let mut inc_b = 0;

    loop {
        print!("{}: ", username().green().bold(),);

        // 文字色処理アルゴリズム ---------------------------------
        let dir_s = get_current_dir_as_vec();
        //inc_r = r / dir_s.len();
        inc_g = r / dir_s.len();
        inc_b = b / dir_s.len();

        for i in dir_s {
            //path_base_color.r += inc_r as u8;
            path_base_color.g += inc_g as u8;
            path_base_color.b += inc_b as u8;
            print!("{}/", i.custom_color(path_base_color));
        }
        path_base_color = CustomColor::new(0, 0, 0);
        // --------------------------------------------------------
        print!(" {} ", cursor);

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
    let _ = rhs_loop();
}
