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

#[derive(Debug, PartialEq, Clone)]
pub enum StatusCode {
    Success,
    Exit,
}
#[derive(Debug, PartialEq)]
pub struct Status {
    status_code: StatusCode,
    exit_code: i32,
}
impl Status {
    pub fn new(status_code: StatusCode, exit_code: i32) -> Status {
        Status {
            status_code,
            exit_code,
        }
    }
    pub fn success() -> Status {
        Status {
            status_code: StatusCode::Success,
            exit_code: 0,
        }
    }
    /*
    pub fn notfound() -> Status {
        Status {
            status_code: StatusCode::NotFound,
            exit_code: 101,
        }
    }*/
    pub fn get_status_code(&self) -> StatusCode {
        self.status_code.clone()
    }
    pub fn get_exit_code(&self) -> i32 {
        self.exit_code
    }
}
