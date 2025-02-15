use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::io::{self, BufRead};

#[derive(PartialEq, Clone)]

pub struct History {
    command: String,
    time: String,
}

impl History {
    pub fn new(command: String, time: String) -> Self {
        Self { command, time }
    }
    pub fn get_command(&self) -> &String {
        &self.command
    }
    pub fn get_time(&self) -> &String {
        &self.time
    }
}

pub fn csv_writer(command: String, time: String, path: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new().append(true).create(true).open(path)?;

    writeln!(file, "{},{}", command, time)?;
    file.flush().unwrap();
    Ok(())
}

pub fn csv_reader(path: &str) -> io::Result<Vec<History>> {
    let file = File::open(path)?;
    let reader = io::BufReader::new(file);

    let mut records: Vec<History> = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() == 2 {
            records.push(History {
                command: parts[0].to_string(),
                time: parts[1].to_string(),
            });
        }
    }

    Ok(records)
}
