use crate::{RshError, Status};
use nix::unistd::*;
use std::path::Path;

pub fn rsh_cd(dir: &str) -> Result<Status, RshError> {
    if !dir.is_empty() {
        // TODO: エラーハンドリング
        chdir(Path::new(dir))
            .map(|_| Status::Success)
            .map_err(|err| RshError::new(&err.to_string()))
    } else {
        Err(RshError::new("rsh: expected arguments to cd\n"))
    }
}
