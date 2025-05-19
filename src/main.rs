mod log;
mod cli;
mod commands;
mod git;
mod util;
mod suse;
use crate::log::*;
use crate::cli::*;
use crate::commands::*;
use crate::util::*;
use crate::suse::cmd_suse;
use clap::ArgMatches;
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
    pub after:          Option<String>,
}

impl Options {
    pub fn new() -> Options {
        Options {
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
            after: None,
        }
    }

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

        let append_matches = matches.subcommand_matches("append");
        if append_matches.is_some() {
            let hash = append_matches.unwrap().get_one::<String>("hashes to append").cloned();
            if hash.is_some() { self.hash = hash }
        }

        let insert_matches = matches.subcommand_matches("insert");
        if insert_matches.is_some() {
            let hash = insert_matches.unwrap().get_one::<String>("hashes to append").cloned();
            if hash.is_some() { self.hash = hash }

            let after = insert_matches.unwrap().get_one::<String>("insert after this hash").cloned();
            if after.is_some() { self.after = after }
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
    let mut log = Log::new();

    let mut command = Cli::parse();
    let matches = command.clone().get_matches();
    if let Some(_matches) = matches.subcommand_matches("setup") {
        cmd_setup(&matches)?;
    }

    log.load()?;
    let mut options = Options::new();

    options.parse(&matches, &log)?;

    if matches.get_flag("debug") {
        println!("Options: {:?}", options);
    }

    // Check for all required options
    if options.range_start.is_none() {
        return Err("Missing option: range-start".into());
    }
    if options.range_stop.is_none() {
        return Err("Missing option: range-stop".into());
    }
    if options.branch.is_none() {
        return Err("Missing option: branch".into());
    }
    if options.git_dir.is_none() {
        return Err("Missing option: git-dir".into());
    }

    // Set defaults to missing options
    if options.work_dir.is_none() {
        options.work_dir = Some("./".to_string());
    }
    if options.paths.is_none() {
        options.paths = Some("/".to_string());
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
    } else if let Some(_matches) = matches.subcommand_matches("diffdiff") {
        cmd_diffdiff(&options)?;
    } else if let Some(_matches) = matches.subcommand_matches("diffstat") {
        cmd_diffstat(&options)?;
    } else if let Some(_matches) = matches.subcommand_matches("rebase") {
        cmd_rebase(&options, &mut log)?;
    } else if let Some(_matches) = matches.subcommand_matches("prepend") {
        cmd_prepend(&options, &mut log)?;
    } else if let Some(_matches) = matches.subcommand_matches("append") {
        cmd_append(&options, &mut log)?;
    } else if let Some(_matches) = matches.subcommand_matches("insert") {
        cmd_insert(&options, &mut log)?;
    } else if let Some(suse_matches) = matches.subcommand_matches("suse") {
        let subcommand = command.find_subcommand_mut("suse").unwrap();
        cmd_suse(&options, log, subcommand, suse_matches)?;
    } else {
        let _ = command.print_help();
    }

    Ok(())
}
