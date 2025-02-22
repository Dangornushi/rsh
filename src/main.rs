mod command;
mod error;
mod evaluator;
mod log;
mod parser;
mod rsh;

use crate::rsh::rsh::Rsh;
use crossterm::{
    cursor::MoveToColumn,
    execute,
    style::{Color, Print, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use std::io::stdout;

fn main() {
    enable_raw_mode().unwrap();
    let mut rsh = Rsh::new();
    let code = rsh.rsh_loop();
    disable_raw_mode().unwrap();
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
