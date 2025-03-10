mod log;
mod cli;
mod commands;
use crate::log::*;
use crate::cli::*;
use clap::{ArgMatches};
use std::fmt::Debug;
use std::error::Error;
use crate::commands::*;

#[derive(Debug)]
pub struct Options {
    pub range_start:    Option<String>,
    pub range_stop:     Option<String>,
    pub branch:         Option<String>,
    pub branch_point:   Option<String>,
    pub work_dir:       Option<String>,
    pub git_dir:        Option<String>,
    pub paths:          Option<String>,
    pub signature:      Option<String>,
    pub references:     Option<String>,
}

impl Options {
    pub fn parse_matches(&mut self, matches: &ArgMatches) {
        let range_start = matches.get_one::<String>("first commit").cloned();
        let range_stop = matches.get_one::<String>("last commit").cloned();
        let branch = matches.get_one::<String>("branch name").cloned();
        let branch_point = matches.get_one::<String>("branch point").cloned();
        let work_dir = matches.get_one::<String>("work directory").cloned();
        let git_dir = matches.get_one::<String>("git directory").cloned();
        let paths = matches.get_one::<String>("paths").cloned();
        let signature = matches.get_one::<String>("signature").cloned();
        let references = matches.get_one::<String>("patch references").cloned();

        if !range_start.is_none() { self.range_start = range_start }
        if !range_stop.is_none() { self.range_stop = range_stop }
        if !branch.is_none() { self.branch = branch }
        if !branch_point.is_none() { self.branch_point = branch_point }
        if !work_dir.is_none() { self.work_dir = work_dir }
        if !git_dir.is_none() { self.git_dir = git_dir }
        if !paths.is_none() { self.paths = paths }
        if !signature.is_none() { self.signature = signature }
        if !references.is_none() { self.references = references }
    }
}

fn parse_options(matches :&ArgMatches, log :&Log) -> Result<Options, Box<dyn Error>> {
    let mut options = Options {
        range_start: None,
        range_stop:  None,
        branch: None,
        branch_point: None,
        work_dir: None,
        git_dir: None,
        paths: None,
        signature: None,
        references: None,
    };

    // Check the local config file
    log.parse_config(&mut options)?;

    // Parse options from cli
    options.parse_matches(&matches);

    // println!("debug is {}", matches.get_flag("debug"));

    // Then check the users global config file

    Ok(options)
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut log = Log {
        filename: String::from("commits.log"),
        config: String::from(""),
        commits: String::from("")
    };

    log.load()?;
    let matches = Cli::parse_command_line();
    let options = parse_options(&matches, &log)?;
    println!("{:?}", options);

    if let Some(_matches) = matches.subcommand_matches("export") {
        cmd_export(&options);
    }

    Ok(())
}
