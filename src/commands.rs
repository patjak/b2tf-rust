use std::process::Command;
use std::error::Error;
use crate::Options;
use crate::Log;
use crate::git::Git;

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

    println!("{summary}");

    Ok(())
}
