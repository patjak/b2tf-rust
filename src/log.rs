#![allow(unused)]
use std::fs;
use std::error::Error;
use crate::cli::*;
use crate::Options;

#[derive(Debug, Clone)]
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

    // Update backport id for upstream id in the log
    pub fn commit_update(&mut self, upstream_id: &str, backport_id: &str) -> Result<(), Box<dyn Error>> {
        let lines: Vec<&str> = self.commits.split("\n").collect();
        let mut commits = String::from("");

        for line in lines.iter() {
            let cols: Vec<&str> = line.trim().split(" ").collect();

            if cols[0] == upstream_id {
                commits.push_str(upstream_id);
                commits.push(' ');
                commits.push_str(backport_id);
                commits.push('\n');
            } else {
                commits.push_str(line);
                commits.push('\n');
            }
        }
        self.commits = commits;
        self.save()?;
        Ok(())
    }

    // Returns the next commit to apply
    pub fn next_commit(&self) -> &str {
        let lines: Vec<&str> = self.commits.split("\n").collect();
        let mut hash: &str = "";

        for line in lines.iter() {
            hash = "";
            let line = line.trim();

            if line.is_empty() { continue; }
            if &line[0..1] == "#" { continue; }

            let rows: Vec<&str> = line.split(" ").collect();
            if rows.len() >= 2 { continue; }
            if rows[0].len() != 40 { continue; }

            hash = rows[0];

            break;
        }
        hash
    }

    // Returns the index of the next commit to apply
    pub fn next_index(&self) -> u32 {
        let lines: Vec<&str> = self.commits.split("\n").collect();
        let mut i: u32 = 0;

        for line in lines.iter() {
            let line = line.trim();

            if line.is_empty() { continue; }
            if &line[0..1] == "#" { continue; }

            let rows: Vec<&str> = line.split(" ").collect();
            i += 1;
            if rows.len() >= 2 { continue; }
            if rows[0].len() != 40 { continue; }

            break;
        }
        i
    }

    // Return the number of commits in the list
    pub fn num_commits(&self) -> Result<u32, Box<dyn Error>> {
        let lines: Vec<&str> = self.commits.split("\n").collect();

        let mut num: u32 = 0;

        for line in lines.iter() {
            if line.trim() == "" { continue; }

            let cols: Vec<&str> = line.split(" ").collect();
            if cols[0] == "#" { continue; }

            if cols[0].len() == 40 { num += 1; }
        }

        Ok(num)
    }
}
