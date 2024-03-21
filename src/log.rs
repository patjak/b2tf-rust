#![allow(unused)]
use std::fs;
use std::error::Error;

pub struct Log {
    pub filename: String,
    pub config: String,
    pub commits: String,
}

impl Log {
    pub fn load(&mut self) -> Result<(), Box<dyn Error>>  {
        let contents: String = fs::read_to_string(&self.filename)?.parse()?;
        let slices: Vec<&str> = contents.split("\n---\n").collect();

        self.config = String::from(slices[0]);
        self.commits = String::from(slices[1]);

        if slices.len() != 2 {
            return Err("Log::Load() Invalid format".into());
        }

        Ok(())
    }

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let contents = format!("{}\n---\n{}", self.config, self.commits);
        fs::write(&self.filename, contents)?;

        Ok(())
    }
}
