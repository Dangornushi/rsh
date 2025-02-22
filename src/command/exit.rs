use crate::error::error::{RshError, Status, StatusCode};

pub fn rsh_exit() -> Result<Status, RshError> {
    println!("Bye");
    Ok(Status::new(StatusCode::Exit, 0))
}
