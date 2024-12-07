use crate::{RshError, Status};

pub fn rsh_exit() -> Result<Status, RshError> {
    println!("Bye");
    Ok(Status::Exit)
}