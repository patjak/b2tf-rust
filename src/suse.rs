use std::process::Command as Cmd;
use std::error::Error;
use std::{fs, io};
use std::path;
use std::path::*;
use crate::Options;
use crate::Log;
use crate::git::Git;
use crate::Util;
use crate::commands::*;
use clap::{ArgMatches, Command};
use colored::Colorize;

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
        Some(("apply", _sub_m)) => {
            cmd_suse_apply(options)?;
        },
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

        // Get downstream hash from "From" header
        let mut contents: String = fs::read_to_string(&file_path)?.parse()?;
        let mut lines: Vec<&str> = contents.split("\n").collect();
        let mut cols: Vec<&str> = lines[0].split(" ").collect();
        if cols[0] != "From" || cols[1].len() != 40 {
            return Err(format!("Invalid patch file: {}", file_path).into());
        }

        // Update "From" header with upstream hash
        let hash_up = log.get_upstream(cols[1])?;
        cols[1] = &hash_up;
        let line = cols.join(" ");
        lines[0] = &line;
        contents = lines.join("\n");
        fs::write(&file_path, contents)?;

        // Add Git-commit tag
        add_suse_tag(&file_path, &kernel_source, "Git-commit", &hash_up)?;

        // Add mainline tag
        let mainline = get_mainline_tag(&hash_up, &git_dir)?;
        add_suse_tag(&file_path, &kernel_source, "Patch-mainline", &mainline)?;

        // Add Acked-by tag
        add_suse_tag(&file_path, &kernel_source, "Acked-by", &signature)?;

        // Add References tag
        add_suse_tag(&file_path, &kernel_source, "References", &references)?;
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
        .expect("Failed to set tag");

    let stderr = String::from_utf8(output.stderr).expect("Invalid UTF8");

    if !output.status.success() {
        println!("{}", stderr);
        return Err("Failed to set SUSE tag".into());
    }

    Ok(())
}

fn add_suse_tag(file_path: &String, kernel_source: &String, tag: &str, value: &str) -> Result <(), Box<dyn Error>> {
    let query = format!("{}/scripts/patch-tag --Add {}='{}' {}", kernel_source, tag, value, file_path);
    let output = Cmd::new("sh")
        .arg("-c")
        .arg(query)
        .output()
        .expect("Failed to add tag");

    let stderr = String::from_utf8(output.stderr).expect("Invalid UTF8");

    if !output.status.success() {
        println!("{}", stderr);
        return Err("Failed to add SUSE tag".into());
    }

    Ok(())
}

fn get_mainline_tag(hash: &str, git_dir: &String) -> Result<String, Box<dyn Error>> {
    let line = Git::cmd(format!("describe --contains --match 'v*' {}", hash), git_dir)?;
    let tag: Vec<&str> = line.split("~").collect();
    let mainline = tag[0].to_string();

    Ok(mainline)
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

fn copy_alt_commits(src: &String, dst: &String, kernel_source: &String) -> Result<(), Box<dyn Error>> {
    // If there is no source file we do nothing
    if !fs::exists(src)? {
        return Ok(());
    }

    let dst_commits = get_git_commits_from_patch(dst)?;
    let src_commits = get_git_commits_from_patch(src)?;

    // Transfer all non git-commit hashes from src to dst as alt-commits
    for d in src_commits {
        if !dst_commits.contains(&d) {
            println!("{} {}", "Adding Alt-commit: ".yellow(), &d.yellow());
            add_suse_tag(dst, kernel_source, "Alt-commit", &d)?;
        }
    }

    Ok(())
}

fn copy_patch(src: &String, dst: &String, kernel_source: &String) -> Result<(), Box<dyn Error>> {
    // Always copy the references so they are never lost
    copy_references(dst, src, kernel_source)?;

    // Copy any altenative hashes for this patch
    copy_alt_commits(dst, src, kernel_source)?;

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

fn series_sort(kernel_source: &String) -> Result<(), Box<dyn Error>> {
    let status = Cmd::new("sh")
        .arg("-c")
        .arg(format!("cd {} && scripts/git_sort/series_sort", kernel_source))
        .status()
        .expect("Failed to sort series.conf");

    if !status.success() {
        return Err("Failed to sort series.conf".into());
    }

    Ok(())
}

fn sequence_patch(kernel_source: &String, file_name: &String, paths: &Vec<PathBuf>, processed_commits: &mut Vec<Vec<String>>) -> Result<(), Box<dyn Error>> {
    'outer: loop {
        let output = Cmd::new("sh")
            .arg("-c")
            .arg(format!("cd {} && scripts/sequence-patch --dry --rapid", kernel_source))
            .output()
            .expect("Failed to sequence patches");

        let stderr = String::from_utf8(output.stderr).expect("Invalid UTF8");

        if output.status.success() {
            break;
        }

        let hunk: Vec<&str> = stderr.split("\nPatch ").collect();
        let lines: Vec<&str> = hunk[1].split("\n").collect();
        let cols: Vec<&str> = lines[0].split(" ").collect();
        let failed_patch = cols[0];

        println!("{} {}", "Sequence failed with patch:".red(), failed_patch.red());

        // If sequencing fails we check if the failed patch is going to be applied
        // by us later. And if so, we automatically guard it.
        let failed_hashes = get_git_commits_from_patch(&format!("{}/{}", &kernel_source, &failed_patch))?;
        'inner: for path in paths {
            let hashes = get_git_commits_from_patch(&path.display().to_string())?;

            if compare_commits(&hashes, &failed_hashes) {
                 for p in &mut *processed_commits {
                    if compare_commits(&hashes, &p) {
                        // We've already processed this patch so don't try to guard it
                        // automatically
                        break 'inner;
                    }
                }
                println!("{}", "Automatically guarding patch since it will be applied later".yellow());
                let file_name = failed_patch.split("/").collect::<Vec<&str>>().clone();
                let file_name = file_name.last().unwrap();
                insert_guard(file_name, kernel_source, processed_commits)?;
                continue 'outer;
            }
        }

        let ask = Util::ask("(R)etry, (g)uard, (v)iew, (a)bort: ".to_string(), vec!["r", "g", "v", "a"], "r");

        match ask.as_str() {
            "r" => (),
            "g" => {
                let file_name = failed_patch.split("/").collect::<Vec<&str>>().clone();
                let file_name = file_name.last().unwrap();
                insert_guard(file_name, kernel_source, processed_commits)?;
            },
            "v" => {
                Cmd::new("sh")
                    .arg("-c")
                    .arg(format!("vim {}/{}", kernel_source, failed_patch))
                    .status()
                    .expect("Failed to open failed patch file");
                continue;
            },
            "a" => {
                series_remove(kernel_source, file_name)?;
                return Err("Aborted by user".into());
            },
            _ => (),
        }
    }

    Ok(())
}

fn series_insert(kernel_source: &String, file_name: &String) -> Result<(), Box<dyn Error>> {
    let path = format!("patches.suse/{}", file_name);
    let query = format!("cd {} && scripts/git_sort/series_insert {} && git add {}",
                        kernel_source, path, path);
    let output = Cmd::new("sh")
        .arg("-c")
        .arg(query)
        .output()
        .expect("series insert failed");

    let stderr = String::from_utf8(output.stderr).expect("Invalid UTF8");

    if !output.status.success() {
        println!("{}", stderr);
        return Err("series insert failed".into());
    }

    Ok(())
}

fn series_remove(kernel_source: &String, file_name: &String) -> Result<(), Box<dyn Error>> {
    let path = format!("patches.suse/{}", file_name);
    let query = format!("cd {} && git restore --staged {} && git restore series.conf && rm {}",
                        kernel_source, path, path);
    let output = Cmd::new("sh")
        .arg("-c")
        .arg(query)
        .output()
        .expect("series remove failed");

    let stderr = String::from_utf8(output.stderr).expect("Invalid UTF8");

    if !output.status.success() {
        println!("{}", stderr);
        return Err("series remove failed".into());
    }

    Ok(())
}

fn suse_log(kernel_source: &String, msg: &str) -> Result<(), Box<dyn Error>> {
    // If only series.conf is modified we are unguarding and scripts/log doesn't work
    let session = Git::get_session(&kernel_source)?;
    if session.unmerged_paths.len() == 0 &&
       session.unstaged_paths.len() == 0 &&
       session.modified_paths.len() == 1 &&
       session.modified_paths[0].1 == "series.conf" {
        Git::cmd("add series.conf".to_string(), &kernel_source)?;
        Git::cmd(format!("commit -m 'Remove guard from {}'", msg), &kernel_source)?;
        println!("Commited unguarding");
    }

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
fn insert_guard(file_name: &str, kernel_source: &String, processed_commits: &mut Vec<Vec<String>>) -> Result<(), Box<dyn Error>> {
    let series_path = format!("{}/series.conf", kernel_source);
    let path = format!("patches.suse/{}", file_name);
    let file = fs::read_to_string(&series_path)?;
    let lines: Vec<&str> = file.split("\n").collect();

    // Check series.conf if patch is already guarded
    if check_guard(file_name, kernel_source)?.is_some() {
        return Err("Patch is already guarded".into());
    }

    // Make sure we're not guarding an already processed commit
    let hashes = get_git_commits_from_patch(&format!("{}/{}", kernel_source, path))?;
    for g in &mut *processed_commits {
        if compare_commits(&hashes, &g) {
            println!("{}", "Patch has already been processed and cannot be guarded. Patch must be fixed instead.".red());
            return Ok(());
        }
    }

    let mut result_str = String::new();
    let mut guard_str = String::new();

    for line in lines {
        let l = line.trim();
        let cols: Vec<&str> = l.split("\t").collect();

        if cols.len() == 1 && cols[0] == path {
            println!("{} {}", "Adding guard +b2tf to".yellow(), path.yellow());
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

        if cols.len() == 2 && cols[0] == "+b2tf" && cols[1] == path {
            continue;
        }
        result_str.push_str(format!("{}\n", line).as_str());
    }

    fs::write(&series_path, result_str)?;

    Ok(())
}

fn replace_patch(file_path: &String, suse_path: &String, kernel_source: &String,
                 always_replace: &mut Vec<String>, never_replace: &mut Vec<String>) -> Result<bool, Box<dyn Error>> {

    let file_name = suse_path.split("/").collect::<Vec<&str>>().clone();
    let file_name = file_name.last().unwrap();

    print!("{} {}", "Found existing patch:".yellow(), &file_name.yellow());

    let refs = get_suse_tags(&suse_path, &kernel_source, "References")?;
    if refs.len() < 1 {
        return Err("Missing references tag".into());
    }
    let refs: Vec<&str> = refs[0].split(" ").collect();
    println!(" ({})", &refs.join(" "));

    // If any of the refs are not to be replaced we do nothing
    for r in &refs {
        if never_replace.contains(&r.to_string()) {
            println!("Skipping patch due to reference: {}", r);
            return Ok(true);
        }
    }

    match compare_patches(suse_path, file_path)? {
        CompareResult::Different => println!("Patches are different"),
        CompareResult::Similar => println!("Patches have the same changes but at different lines"),
        CompareResult::Same => println!("Patches have identical changes but have other differences"),
        CompareResult::Identical => println!("Patches are identical"),
    }

    let mut handled = false;
    for r in &refs {
        if always_replace.contains(&r.to_string()) {
            continue;
        }

        println!("{} {}", "Replace patch with reference:".green(), r.green().bold());
        print!("{}", get_ref_link(&r).yellow());

        loop {
            let ask = Util::ask("(Y)es, (n)o, (a)lways, n(e)ver, (v)iew, or (s)top: ".to_string(),
                                vec!["y", "n", "a", "e", "v", "s"], "y");

            // FIXME: Support other editors
            match ask.as_str() {
                "y" => {
                    // If all refs are yes we replace the patch
                    break;
                },
                "n" => {
                    // If a single ref is no we don't replace the patch
                    handled = true;
                    break;
                },
                "a" => {
                    always_replace.push(r.to_string());
                    break;
                },
                "e" => {
                    never_replace.push(r.to_string());
                    handled = true;
                    break;
                },
                "v" => {
                    Cmd::new("sh")
                        .arg("-c")
                        .arg(format!("diff -Naur {} {} > /tmp/{}.patch || vim -O {} {} /tmp/{}.patch && rm /tmp/{}.patch",
                             suse_path, file_path, file_name, suse_path, file_path, file_name, file_name))
                        .status()
                        .expect("Failed to show diff");
                },
                "s" => {
                    return Err("Stopped by user".into());
                },
                _ => (),
            };
        }

    }

    Ok(handled)
}

pub fn cmd_suse_apply(options: &Options) -> Result<(), Box<dyn Error>> {
    let work_dir = options.work_dir.clone().unwrap();
    let kernel_source = options.kernel_source.clone().unwrap();

    println!("Applying patches to SUSE kernel-source...\n");

    // Gather all commits from kernel_source as (file_path, git-commit)
    let mut suse_paths: Vec<(String, Vec<String>)> = vec![];
    let paths = fs::read_dir(format!("{}/patches.suse/", kernel_source))?;
    for path in paths {
        let file_path = path?.path().display().to_string().clone();
        let git_commits = get_git_commits_from_patch(&file_path)?.clone();
        suse_paths.push((file_path, git_commits));
    }

    let mut paths = fs::read_dir(format!("{}/patches.suse/", work_dir))?
                    .map(|res| res.map(|e| e.path()))
                    .collect::<Result<Vec<_>, io::Error>>()?;
    paths.sort();

    let mut i = 1;
    let total = paths.len();

    // Store the choices made by the user
    let mut always_replace: Vec<String> = vec![];
    let mut never_replace: Vec<String> = vec![];

    // Remeber what we've processed so we don't end up in a guard/unguard loop
    let mut processed_commits: Vec<Vec<String>> = vec![];

    // Loop over all backported patches
    for path in &paths {
        let file_path = path.display().to_string();
        let file_name = file_path.split("/").collect::<Vec<&str>>().clone();
        let file_name = file_name.last().unwrap();
        let git_commits = get_git_commits_from_patch(&file_path)?;
        processed_commits.push(git_commits.clone());

        println!("Progress:\t{}/{}\t{}", i, total, file_name);
        i += 1;

        let mut handled = false;
        let mut unguarding = false;
        // Loop over all already existing SUSE backports
        for suse_path in &suse_paths {
            if !compare_commits(&git_commits, &suse_path.1) {
                continue;
            }

            // If the patch is guarded by us we must unguard it first
            let guard_file_name = suse_path.0.split("/").collect::<Vec<&str>>().clone();
            let guard_file_name = guard_file_name.last().unwrap();
            let guard = check_guard(guard_file_name, &kernel_source)?;
            if guard.is_some() && guard.unwrap() == "+b2tf" {
                println!("{} {}", "Unguarding".yellow(), guard_file_name.yellow());
                remove_guard(&guard_file_name, &kernel_source)?;
                series_insert(&kernel_source, &guard_file_name.to_string())?;
                unguarding = true;
            }

            let comp_res = compare_patches(&file_path, &suse_path.0)?;

            if comp_res == CompareResult::Identical || comp_res == CompareResult::Same {
                let query = format!("ls-files --error-unmatch {} > /dev/null", suse_path.0);
                if !Git::cmd_passthru(query, &kernel_source)? {
                    return Err("Patch was applied but not committed. Fix the state of the kernel-source before continuing".into());
                }

                if unguarding {
                    println!("Patch is same or identical. Sequencing...");
                    sequence_patch(&kernel_source, &file_name.to_string(), &paths, &mut processed_commits)?;
                    suse_log(&kernel_source, &suse_path.0)?;
                    handled = true;
                } else {
                    println!("Patch is same or identical. Skipping.");
                    handled = true;
                }
                break;
            }

            handled = replace_patch(&file_path, &suse_path.0, &kernel_source, &mut always_replace, &mut never_replace)?;
            if !handled {
                copy_patch(&file_path, &suse_path.0, &kernel_source)?;

                // Git-commit and Alt-commit might have changed places so update series.conf
                series_sort(&kernel_source)?;

                sequence_patch(&kernel_source, &file_name.to_string(), &paths, &mut processed_commits)?;
                suse_log(&kernel_source, &suse_path.0)?;
                handled = true;
            }
        }

        if handled {
            continue;
        }

        let dst_path = format!("{}/patches.suse/{}", kernel_source, file_name);

        copy_patch(&file_path, &dst_path, &kernel_source)?;
        series_insert(&kernel_source, &file_name.to_string())?;
        sequence_patch(&kernel_source, &file_name.to_string(), &paths, &mut processed_commits)?;
        suse_log(&kernel_source, "")?;
    }

    println!("\nDone");

    Ok(())
}
