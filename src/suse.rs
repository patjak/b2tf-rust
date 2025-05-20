use std::error::Error;
use std::path;
use crate::Options;
use crate::Log;
use crate::git::Git;
use clap::{ArgMatches, Command};

pub fn cmd_suse(options: &mut Options, log: &Log, subcommand: &mut Command, matches: &ArgMatches) -> Result<(), Box<dyn Error>> {
    let signature = matches.get_one::<String>("signature").cloned();
    let references = matches.get_one::<String>("patch references").cloned();
    let kernel_source = matches.get_one::<String>("Path to SUSE kernel-source").cloned();

    if signature.is_some() {
        options.signature = signature;
    } else if options.signature.is_none() {
        return Err("suse subcommands requires option --signature to be specified".into());
    }

    if references.is_some() {
        options.references = references;
    } else if options.references.is_none() {
        return Err("suse subcommands require option --references to be specified".into());
    }

    if kernel_source.is_some() {
        options.kernel_source = kernel_source;
    } else if options.kernel_source.is_none() {
        return Err("suse subcommands require option --suse-kernel-source to be specified".into());
    }

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
