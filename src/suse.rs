use std::error::Error;
use crate::Options;
use crate::Log;
use clap::{ArgMatches, Command};

pub fn cmd_suse(_options: &Options, _log: Log, subcommand: &mut Command, matches: &ArgMatches) -> Result<(), Box<dyn Error>> {

    match matches.subcommand() {
        Some(("export", _sub_m)) => {},
        Some(("unblacklist", _sub_m)) => {},
        Some(("replace", _sub_m)) => {},
        Some(("apply", _sub_m)) => {},
        Some((&_, _)) => {},
        None => {let _ = subcommand.print_help();},
    }

    Ok(())
}
