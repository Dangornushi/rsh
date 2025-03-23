use crate::error::error::{RshError, Status};
pub fn rsh_sort(arg: Vec<String>) -> Result<Status, RshError> {
    println!("sort: {:?}", arg);
    Ok(Status::success())
}
