use crate::error::error::{RshError, Status};
use nix::unistd::*;
use std::path::Path;

pub fn rsh_sort(arg: Vec<String>) -> Result<Status, RshError> {
    println!("sort: {:?}", arg);
    Ok(Status::success())
}
