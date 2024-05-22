mod log;
mod cli;
use crate::log::*;
use crate::cli::*;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut log = Log {
        filename: String::from("commits.log"),
        config: String::from(""),
        commits: String::from("")
    };

    let options = Cli::parse_command_line();

    log.load()?;

    Ok(())
}
