mod log;
use crate::log::*;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut log = Log {
        filename: String::from("commits.log"),
        config: String::from(""),
        commits: String::from("")
    };

    log.load()?;

    Ok(())
}
