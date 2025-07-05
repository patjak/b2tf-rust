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
        Some(("unblacklist", _sub_m)) => {
            cmd_suse_unblacklist(options)?;
        },
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

        let stderr = String::from_utf8(output.stderr).expect("Invalid UTF8");

        if !output.status.success() {
            println!("{}", stderr);
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

// Remove a blacklist entry from a specified blacklist.conf file
fn remove_blacklist_entry(hash: &str, kernel_source: &str) -> Result<(), Box<dyn Error>> {
    let file_path = format!("{}/blacklist.conf", kernel_source);
    let contents: String = fs::read_to_string(&file_path)?.parse()?;
    let mut output: Vec<&str> = vec![];

    let lines: Vec<&str> = contents.split("\n").collect();
    for line in lines {
        let cols: Vec<&str> = line.split(" ").collect();

        if cols.len() > 0 && cols[0] == hash {
            println!("Removed blacklist entry:\n{}", line);
            continue;
        }

        output.push(line);
    }

    let output = output.join("\n");
    fs::write(&file_path, output)?;

    Ok(())
}

// Returns all git-commit and alt-commit tags from patch
fn get_git_commits_from_patch(file_path: &String) -> Result<Vec<String>, Box<dyn Error>> {
    if !fs::exists(file_path)? {
        return Err(format!("File not found: {}", file_path).into());

    }

    let mut hashes: Vec<String> = Vec::new();

    let mut contents: String = fs::read_to_string(file_path)?.parse()?;
    contents = contents.to_lowercase();

    let lines: Vec<&str> = contents.split("\n").collect();
    for line in lines {
        // Stop parsing when the diff starts
        if line == "---" {
            break;
        }
        let hash: Vec<&str> = line.split("git-commit: ").collect();
        if hash.len() == 2 && hash[1].len() == 40 {
            hashes.push(hash[1].to_string());
        }

        let hash: Vec<&str> = line.split("alt-commit: ").collect();
        if hash.len() == 2 &&  hash[1].len() == 40 {
            hashes.push(hash[1].to_string());
        }
    }

    Ok(hashes)
}

fn compare_commits(list1: &Vec<String>, list2: &Vec<String>) -> bool {
    for h in list1 {
        if list2.contains(h) {
            return true;
        }
    }

    false
}

pub fn cmd_suse_unblacklist(options: &Options) -> Result<(), Box<dyn Error>> {
    let work_dir = options.work_dir.clone().unwrap();
    let kernel_source = options.kernel_source.clone().unwrap();

    println!("Removing blacklist entries in blacklist.conf...");

    let mut paths = fs::read_dir(format!("{}/patches.suse/", work_dir))?
                    .map(|res| res.map(|e| e.path()))
                    .collect::<Result<Vec<_>, io::Error>>()?;
    paths.sort();

    for path in paths {
        let file_path = path.display().to_string();
        let git_commits = get_git_commits_from_patch(&file_path)?;
        for hash in git_commits {
            remove_blacklist_entry(&hash, &kernel_source)?;
        }
    }

    Ok(())
}

fn get_suse_tags(file_path: &String, kernel_source: &String, tag: &str) -> Result <Vec<String>, Box<dyn Error>> {
    let output = Cmd::new("sh")
        .arg("-c")
        .arg(format!("{}/scripts/patch-tag --print {} {}", kernel_source, tag, file_path))
        .output()
        .expect("Failed to get tag");

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF8");
    let lines: Vec<&str> = stdout.split("\n").collect();
    let mut tags = Vec::new();
    let tag_str = format!("{}: ", tag);

    for line in lines {
        let cols: Vec<&str> = line.split(&tag_str).collect();
        if cols.len() != 2 {
            continue;
        }
        tags.push(cols[1].to_string());
    }

    Ok(tags)
}

fn set_suse_tag(file_path: &String, kernel_source: &String, tag: &str, value: &str) -> Result <(), Box<dyn Error>> {
    let query = format!("{}/scripts/patch-tag --tag {}='{}' {}", kernel_source, tag, value, file_path);
    let output = Cmd::new("sh")
        .arg("-c")
        .arg(query)
        .output()
        .expect("Failed to get tag");

    let stderr = String::from_utf8(output.stderr).expect("Invalid UTF8");

    if !output.status.success() {
        println!("{}", stderr);
        return Err("Failed to set SUSE tag".into());
    }

    Ok(())
}

fn get_ref_link(r: &str) -> String {
    let t: Vec<&str> = r.split("#").collect();

    if t.len() != 2 {
        return "".to_string();
    }

    let mut link = String::new();

    let link_type = t[0].to_lowercase();
    if link_type == "bsc" {
        link = format!("https://bugzilla.suse.com/show_bug.cgi?id={}\n", t[1]);
    } else if link_type == "jsc" {
        link = format!("https://jira.suse.com/browse/{}\n", t[1]);
    }

    link
}

fn copy_patch(src: &String, dst: &String, kernel_source: &String) -> Result<(), Box<dyn Error>> {
    // Always copy the references so they are never lost
    copy_references(dst, src, kernel_source)?;

    let status = Cmd::new("sh")
        .arg("-c")
        .arg(format!("cp {} {}", src, dst))
        .status()
        .expect("Failed to copy patch");

    if !status.success() {
        return Err("Failed to copy patch".into());
    }

    Ok(())
}

// Adds the contents of the references tag from src_path to dst_path
fn copy_references(src_path: &String, dst_path: &String, kernel_source: &String) -> Result<(), Box<dyn Error>> {
    // If there is no source file we do nothing
    if !fs::exists(src_path)? {
        return Ok(());
    }

    let src_tag = get_suse_tags(src_path, kernel_source, "References")?;
    let dst_tag = get_suse_tags(dst_path, kernel_source, "References")?;

    if src_tag.is_empty() {
        println!("{:?}", src_tag);
        return Err(format!("Source patch didn't have a references tag: {}", src_path).into());
    }
    let src_refs: Vec<&str> = src_tag[0].split(" ").collect();

    if dst_tag.is_empty() {
        return Err("Destination patch didn't have a referecnes tag".into());
    }
    let dst_refs: Vec<&str> = dst_tag[0].split(" ").collect();

    let mut result_str = String::new();

    for src_ref in src_refs {
        let mut found = false;
        for dst_ref in &dst_refs {
            if src_ref == *dst_ref {
                found = true;
                break;
            }
        }
        // If reference is not already in dst we add it
        if !found {
            result_str.push_str(format!("{} ", src_ref).as_str());
        }
    }

    result_str.push_str(dst_tag[0].as_str());
    set_suse_tag(dst_path, kernel_source, "References", result_str.as_str())?;

    Ok(())
}

fn suse_log(kernel_source: &String) -> Result<(), Box<dyn Error>> {
    let output = Cmd::new("sh")
            .arg("-c")
            .arg(format!("cd {} && scripts/log --no-edit", kernel_source))
            .output()
            .expect("Failed to run scripts/log");

    if !output.status.success() {
        let stderr = String::from_utf8(output.stderr).expect("Invalid UTF8");

        println!("{}", stderr);
        return Err("Failed to run scripts/log".into());
    }

    Ok(())
}

fn check_guard(file_name: &str, kernel_source: &String) -> Result<Option<String>, Box<dyn Error>> {
    let series_path = format!("{}/series.conf", kernel_source);
    let path = format!("patches.suse/{}", file_name);
    let file = fs::read_to_string(series_path)?;
    let lines: Vec<&str> = file.split("\n").collect();

    for line in lines {
        let l = line.trim();
        let cols: Vec<&str> = l.split("\t").collect();

        // Check if patch is guarded
        if cols.len() >= 2 && cols[1] == path && cols[0].chars().nth(0).unwrap() == '+' {
            return Ok(Some(cols[0].to_string()));
        }
    }

    Ok(None)
}

// Mark a patch with +b2tf in series.conf
fn insert_guard(file_name: &str, kernel_source: &String) -> Result<(), Box<dyn Error>> {
    let series_path = format!("{}/series.conf", kernel_source);
    let path = format!("patches.suse/{}", file_name);
    let file = fs::read_to_string(&series_path)?;
    let lines: Vec<&str> = file.split("\n").collect();

    if check_guard(file_name, kernel_source)?.is_some() {
        return Err("Patch is already guarded".into());
    }

    let mut result_str = String::new();
    let mut guard_str = String::new();

    for line in lines {
        let l = line.trim();
        let cols: Vec<&str> = l.split("\t").collect();

        if cols.len() == 1 && cols[0] == path {
            println!("Adding guard +b2tf to {}", path);
            guard_str.push_str(format!("+b2tf\t{}\n", path).as_str());
            continue;
        }
        result_str.push_str(format!("{}\n", line).as_str());
    }

    // Remove last newline if needed
    let last_char = result_str.pop().unwrap();
    if last_char != '\n' {
        result_str.push(last_char);
    }

    // Append the guarded patch to the end of series.conf
    result_str.push_str(&guard_str);

    fs::write(&series_path, result_str)?;

    Ok(())
}

// Remove a patch marked with +b2tf in series.conf
fn remove_guard(file_name: &str, kernel_source: &String) -> Result<(), Box<dyn Error>> {
    let series_path = format!("{}/series.conf", kernel_source);
    let path = format!("patches.suse/{}", file_name);
    let file = fs::read_to_string(&series_path)?;
    let lines: Vec<&str> = file.split("\n").collect();

    if !check_guard(file_name, kernel_source)?.is_some() {
        return Err("Patch is not guarded".into());
    }

    let mut result_str = String::new();

    for line in lines {
        let l = line.trim();
        let cols: Vec<&str> = l.split("\t").collect();

        if cols.len() == 1 && cols[0] == path {
            println!("Removing guard +b2tf to {}", path);
            continue;
        }
        result_str.push_str(format!("{}\n", line).as_str());
    }

    fs::write(&series_path, result_str)?;

    Ok(())
}
