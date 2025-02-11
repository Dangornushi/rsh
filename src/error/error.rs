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

#[derive(Debug, PartialEq)]
pub enum Status {
    Success,
    Exit,
}
