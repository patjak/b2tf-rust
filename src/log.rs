#![allow(unused)]
use std::fs;
use std::error::Error;
use crate::cli::*;
use crate::Options;

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

    pub fn parse_config(&self, mut options: &mut Options) -> Result<(), Box<dyn Error>> {
        let configs: Vec<&str> = self.config.split("\n").collect();

        for config in configs.iter() {
            let config: Vec<&str> = config.split(":").collect();
            if config.len() != 2 {
                continue;
            }

            let name: &str = config[0].trim();
            let value: &str = config[1].trim();

            if name == "range-start" {
                options.range_start = Some(value.to_string());

            } else if name == "range-stop" {
                options.range_stop = Some(value.to_string());

            } else if name == "branch" {
                options.branch = Some(value.to_string());

            } else if name == "branch-point" {
                options.branch_point = Some(value.to_string());

            } else if name == "work-dir" {
                options.work_dir = Some(value.to_string());

            } else if name == "git-dir" {
                options.git_dir = Some(value.to_string());

            } else if name == "paths" {
                options.paths = Some(value.to_string());

            } else if name == "signature" {
                options.signature = Some(value.to_string());

            } else if name == "references" {
                options.references = Some(value.to_string());
            }
        };

        Ok(())
    }
}
