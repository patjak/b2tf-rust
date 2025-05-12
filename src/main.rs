mod log;
mod cli;
mod commands;
mod git;
mod util;
use crate::log::*;
use crate::cli::*;
use crate::commands::*;
use crate::util::*;
use clap::{ArgMatches};
use std::fmt::Debug;
use std::error::Error;

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
    pub hash:           Option<String>,
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

        let prepend_matches = matches.subcommand_matches("prepend");
        if prepend_matches.is_some() {
            let hash = prepend_matches.unwrap().get_one::<String>("hashes to prepend").cloned();
            if hash.is_some() { self.hash = hash }
        }

        if range_start.is_some() { self.range_start = range_start }
        if range_stop.is_some() { self.range_stop = range_stop }
        if branch.is_some() { self.branch = branch }
        if branch_point.is_some() { self.branch_point = branch_point }
        if work_dir.is_some() { self.work_dir = work_dir }
        if git_dir.is_some() { self.git_dir = git_dir }
        if paths.is_some() { self.paths = paths }
        if signature.is_some() { self.signature = signature }
        if references.is_some() { self.references = references }
    }

    pub fn parse(&mut self, matches :&ArgMatches, log :&Log) -> Result<(), Box<dyn Error>> {
        // TODO: Check the users global config file

        // Check the local config file
        log.parse_config(self)?;

        // Parse options from cli
        self.parse_matches(matches);

        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut log = Log {
        filename: String::from("b2tf.log"),
        config: String::from(""),
        commits: String::from("")
    };

    log.load()?;
    let matches = Cli::parse();
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
        hash: None,
    };

    options.parse(&matches, &log)?;

    if matches.get_flag("debug") {
        println!("Options: {:?}", options);
    }

    if let Some(_matches) = matches.subcommand_matches("populate") {
        cmd_populate(&options, &mut log)?;
    } else if let Some(_matches) = matches.subcommand_matches("apply") {
        cmd_apply(&options, &mut log)?;
    } else if let Some(_matches) = matches.subcommand_matches("status") {
        cmd_status(&options, &log)?;
    } else if let Some(_matches) = matches.subcommand_matches("edit") {
        cmd_edit(&options, &mut log)?;
    } else if let Some(_matches) = matches.subcommand_matches("restart") {
        cmd_restart(&options, &mut log)?;
    } else if let Some(_matches) = matches.subcommand_matches("skip") {
        cmd_skip(&options, &mut log)?;
    } else if let Some(_matches) = matches.subcommand_matches("diff") {
        cmd_diff(&options)?;
    } else if let Some(_matches) = matches.subcommand_matches("diffstat") {
        cmd_diffstat(&options)?;
    } else if let Some(_matches) = matches.subcommand_matches("rebase") {
        cmd_rebase(&options, &mut log)?;
    } else if let Some(_matches) = matches.subcommand_matches("prepend") {
        cmd_prepend(&options, &mut log)?;
    }

    Ok(())
}
