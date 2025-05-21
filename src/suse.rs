use std::process::Command as Cmd;
use std::error::Error;
use std::{fs, io};
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
        Some(("export", _sub_m)) => {
            cmd_suse_export(options, log)?;
        },
        Some(("unblacklist", _sub_m)) => {},
        Some(("replace", _sub_m)) => {},
        Some(("apply", _sub_m)) => {},
        Some((&_, _)) => {},
        None => {let _ = subcommand.print_help();},
    }

    Ok(())
}

pub fn cmd_suse_export(options: &Options, log: &Log) -> Result<(), Box<dyn Error>> {
    let work_dir = options.work_dir.clone().unwrap();
    let git_dir = options.git_dir.clone().unwrap();
    let branch = options.branch.clone().unwrap();
    let branch_point = options.branch_point.clone().unwrap();
    let work_dir = path::absolute(&work_dir)?.into_os_string().into_string().unwrap();
    let kernel_source = options.kernel_source.clone().unwrap();
    let signature = options.signature.clone().unwrap();
    let references = options.references.clone().unwrap();

    println!("Exporting patches into {}patches.suse/", work_dir);
    Git::cmd(format!("format-patch -o {}/patches.suse/ --no-renames --keep-subject {}..{}",
             work_dir, branch_point, branch), &git_dir)?;

    let mut paths = fs::read_dir(format!("{}/patches.suse/", work_dir))?
                    .map(|res| res.map(|e| e.path()))
                    .collect::<Result<Vec<_>, io::Error>>()?;
    paths.sort();

    let mut i = 0;
    let total = paths.len();

    for path in paths {
        let file_path: String = path.display().to_string();
        let file_name = path.file_name().unwrap().to_str().unwrap();

        i += 1;
        println!("{}/{}:\t{}", i, total, file_name);

        // Update the From header with the upstream hash
        let mut contents: String = fs::read_to_string(&file_path)?.parse()?;
        let mut lines: Vec<&str> = contents.split("\n").collect();
        let mut cols: Vec<&str> = lines[0].split(" ").collect();
        if cols[0] != "From" || cols[1].len() != 40 {
            return Err(format!("Invalid patch file: {}", file_path).into());
        }

        let hash_down = log.get_upstream(cols[1])?;
        cols[1] = &hash_down;
        let line = cols.join(" ");
        lines[0] = &line;
        contents = lines.join("\n");

        fs::write(&file_path, contents)?;

        let query = format!("LINUX_GIT={} {}/scripts/patch-tags-from-git {}",
                            git_dir, kernel_source, file_path);
        let output = Cmd::new("sh")
            .arg("-c")
            .arg(&query)
            .output()
            .expect(format!("Failed to update tags in {}", file_path).as_str());

        if !output.status.success() {
            return Err(format!("Failed to run patch-tags-from-git on: {}", file_path).into());
        }

        // Add Acked-by tag
        let query = format!("{}/scripts/patch-tag --Add \"Acked-by={}\" {}", kernel_source, signature, file_path);
        let output = Cmd::new("sh")
            .arg("-c")
            .arg(&query)
            .output()
            .expect(format!("Failed to execute: {}", query).as_str());

        if !output.status.success() {
            return Err(format!("Failed to add signature to {}", file_path).as_str().into());
        }

        // Add References tag
        let query = format!("{}/scripts/patch-tag --Add \"References={}\" {}", kernel_source, references, file_path);
        let output = Cmd::new("sh")
            .arg("-c")
            .arg(&query)
            .output()
            .expect(format!("Failed to execute: {}", query).as_str());

        if !output.status.success() {
            return Err(format!("Failed to add signature to {}", file_path).as_str().into());
        }
    }
    Ok(())
}
