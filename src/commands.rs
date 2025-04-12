use std::process::Command;
use std::error::Error;
use std::path::Path;
use colored::Colorize;
use crate::Options;
use crate::Log;
use crate::Util;
use crate::git::{Git, GitSession};

pub fn cmd_populate(options: &Options, log: &mut Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let range_start = options.range_start.clone().unwrap();
    let range_stop = options.range_stop.clone().unwrap();
    let paths = options.paths.clone().unwrap();

    let query = format!("git -C {git_dir} rev-list --topo-order --no-merges --oneline --no-abbrev-commit {range_start}..{range_stop} -- {paths}");

    let output = Command::new("sh")
        .arg("-c")
        .arg(&query)
        .output()
        .expect(format!("Failed to execute: {}\n", &query).as_str());

    if !output.status.success() {
        return Err("Failed to execute git rev-list".into());
    }

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF8");
    let lines: Vec<&str> = stdout.split("\n").collect();

    let mut commits = String::from("");

    for line in lines.iter() {
        if line.len() < 43 {
            continue;
        }

        let subject = format!("\n# {}\n", &line[41..]);
        let hash = format!("{}\n", &line[0..40]);

        commits.push_str(subject.as_str());
        commits.push_str(hash.as_str());
    }

    log.commits = commits;
    log.save()?;

    Ok(())
}

pub fn cmd_apply(options: &Options, log: &mut Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let branch = options.branch.clone().unwrap();

    let log_read = log.clone();
    let mut i: u32 = log_read.next_index();
    let num_commits = log_read.num_commits().unwrap();

    Git::set_branch(&branch, &git_dir)?;

    let session = Git::get_session(&git_dir)?;

    if session != GitSession::NONE {
        return Err("In session".into());
    }

    loop {
        let log_read = log.clone();
        let next_hash = log_read.next_commit();
        let commit = Git::log(&next_hash, &git_dir)?;

        println!("Applying {}/{}: {} {}", i, num_commits, next_hash, commit.subject);

        Git::cmd(format!("cherry-pick {}", next_hash), &git_dir)?;
        let new_hash = Git::cmd(format!("log --format='%H' -n 1"), &git_dir)?;

        log.commit_update(next_hash, new_hash.trim())?;
        i += 1;
    }
}

// Returns the line number of the first occurance of '<<<<<<<' in file
fn find_conflict_lineno(file: String) -> Result<String, Box<dyn Error>> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("grep -n '<<<<<<<' {}", file))
        .output()
        .expect("Failed to grep for conflict line");

    if !output.status.success() {
        return Ok("0".into());
    }

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF8");
    let line: Vec<&str> = stdout.split(":").collect();
    let lineno: String = line[0].to_string();

    Ok(lineno)
}

pub fn cmd_edit(options: &Options, log: &Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let range_stop = options.range_stop.clone().unwrap();
    let unmerged_paths = Git::get_unmerged_paths(&git_dir)?;
    let commit = log.next_commit();

    if unmerged_paths.len() > 0 {
        println!("{}", "Unmerged paths:");
        for path in unmerged_paths.iter() {
            println!("\t{}", path.1.red());
        }
        println!("");
    }

    for path in unmerged_paths.iter() {
        let file = &path.1;
        let file_path = Path::new(file);
        let commit_file = format!("/tmp/{}.patch", commit);
        let target_file = format!("/tmp/{}-{}", range_stop, file_path.file_name().unwrap().to_str().unwrap());

        // Store the commit as a patch file
        Git::cmd(format!("show {} > {}", commit, commit_file), &git_dir);

        // Store the target version of the file (eg git show v5.5:<filepath>)
        Git::cmd(format!("show {}:{} > {}", range_stop, file, target_file), &git_dir);

        // Find the line number where to start editing
        let lineno = find_conflict_lineno(format!("{}/{}", git_dir, file)).unwrap();

        // FIXME: Add support for other editors than vim
        Command::new("sh")
            .arg("-c")
            .arg(format!("cd {git_dir} && vim {commit_file} -c 'vs {target_file} | {lineno}' -c 'vs {file} | {lineno}'"))
            .status()
            .expect("Failed to open editor");

        Command::new("sh")
            .arg("-c")
            .arg(format!("rm {commit_file} && rm {target_file}"))
            .status()
            .expect("Failed to remove temporary files");
    }

    Ok(())
}

pub fn cmd_status(options: &Options, log: &Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let branch = options.branch.clone().unwrap();
    let paths = options.paths.clone().unwrap();
    let range_stop = options.range_stop.clone().unwrap();

    Git::set_branch(&branch, &git_dir)?;

    let next_index = log.next_index();
    let num_commits = log.num_commits().unwrap();
    let percentage = (next_index / num_commits) * 100;
    println!("Progress {}% ({}/{})", percentage, next_index, num_commits);

    let stdout = Git::cmd(format!("diff --stat {branch} {range_stop} -- {paths}"), &git_dir).unwrap();
    let lines: Vec<&str> = stdout.split("\n").collect();
    let summary = lines[lines.len() - 2].trim();

    println!("{summary}\n");

    let session = Git::get_session(&git_dir).unwrap();

    match session {
        GitSession::CHERRYPICK => println!("{}", "Session: Cherry-picking".yellow()),
        GitSession::REBASE => println!("{}", "Session: Rebasing\n".yellow()),
        GitSession::NONE => println!("No session"),
    }


    let unmerged_paths = Git::get_unmerged_paths(&git_dir)?;
    let modified_paths = Git::get_modified_paths(&git_dir)?;
    let unstaged_paths = Git::get_unstaged_paths(&git_dir)?;

    if modified_paths.len() > 0 {
        println!("\nChanges to be committed:");
        for path in modified_paths.iter() {
            println!("\t{}", path.1.green());
        }
    }

    if unmerged_paths.len() > 0 {
        println!("\nUnmerged paths:");
        for path in unmerged_paths.iter() {
            println!("\t{}", path.1.red());
        }
    }

    if unstaged_paths.len() > 0 {
        println!("\nChanges not staged for commit:");
        for path in unstaged_paths.iter() {
            println!("\t{}", path.1.red());
        }
    }

    println!("");

    Ok(())
}
