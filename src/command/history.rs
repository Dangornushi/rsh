use crate::{log::log_maneger::History, RshError};


pub fn rsh_history(database: Vec<History>) -> Result<(), RshError> {
    for (_, history) in database.iter().enumerate() {
        println!("{} {}", history.get_time(), history.get_command());
    }
    Ok(())
}
