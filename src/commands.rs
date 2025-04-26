use std::process::Command;
use std::error::Error;
use std::path::Path;
use colored::Colorize;
use crate::Options;
use crate::Log;
use crate::Util;
use crate::git::{Git, GitSessionState};
use patch::{Patch};

pub fn cmd_populate(options: &Options, log: &mut Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let range_start = options.range_start.clone().unwrap();
    let range_stop = options.range_stop.clone().unwrap();
    let paths = options.paths.clone().unwrap();

    let query = format!("rev-list --reverse --topo-order --no-merges --oneline --no-abbrev-commit {range_start}..{range_stop} -- {paths}");
    let stdout = Git::cmd(query, &git_dir)?;
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

// Returns a tuple with hash of commits containing a cherry pick tag and its cherry pick hash
fn get_cherrypick_cache(options: &Options) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let paths = options.paths.clone().unwrap();
    let range_start = options.range_start.clone().unwrap();
    let mut cache: Vec<(String, String)> = vec![];

    let query = format!("log --date=format:%Y-%m-%d --format=%cd -n1 {range_start}").to_string();
    let start_date = Git::cmd(query, &git_dir)?;
    let start_date = start_date.trim();

    let query = format!("log --no-merges --since \"$(date --date \"{start_date} - 6 months\")\" --format='%H' --grep=\"(cherry picked from commit \" -- {paths}").to_string();
    let stdout = Git::cmd(query, &git_dir)?;

    let lines: Vec<&str> = stdout.split("\n").collect();
    for line in lines.iter() {
        if line.len() != 40 {
            continue;
        }
        let hash = line[..40].to_string();
        let commit = Git::show(&hash, &git_dir)?;
        let sections: Vec<_> = commit.body.split("(cherry picked from commit ").collect();
        if sections.len() != 2 {
            return Err("Invalid commit with multiple cherry pick lines".into());
        }
        let section = sections[1];
        let cherrypick = section[..40].to_string();
        if cherrypick.len() != 40 || hash.len() != 40 {
            return Err("NOOOO!".into());
        }
        cache.push((hash, cherrypick));
    }

    Ok(cache)
}

// Return a tuple with (hash, subject) of all commits that can potentially be a cherry pick
fn get_commit_cache(options: &Options) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let paths = options.paths.clone().unwrap();
    let range_start = options.range_start.clone().unwrap();
    let mut cache: Vec<(String, String)> = vec![];

    let query = format!("log --date=format:%Y-%m-%d --format=%cd -n1 {range_start}").to_string();
    let start_date = Git::cmd(query, &git_dir)?;
    let start_date = start_date.trim();

    let query = format!("log --no-merges --since \"$(date --date \"{start_date} - 6 months\")\" --format='%H %s' -- {paths}").to_string();
    let stdout = Git::cmd(query, &git_dir)?;

    let lines: Vec<&str> = stdout.split("\n").collect();
    for line in lines.iter() {
        if line.len() < 41 {
            continue;
        }
        let hash = line[..40].to_string();
        let subject = line[41..].to_string();
        cache.push((hash, subject));
    }

    Ok(cache)
}

fn compare_patches(options: &Options, hash1: &String, hash2: &String) -> Result<bool, Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();

    let commit1 = Git::show(hash1.as_str(), &git_dir)?;
    let commit2 = Git::show(hash2.as_str(), &git_dir)?;
    let patch1 = Patch::from_single(&commit1.body).unwrap();
    let patch2 = Patch::from_single(&commit2.body).unwrap();

    for hunk1 in &patch1.hunks {
        let mut found = false;
        for hunk2 in &patch2.hunks {

            // Compare only the lines and not the ranges since they can vary based on which kernel
            // base they got applied to
            if hunk1.lines == hunk2.lines {
                found = true;
                break;
            }
        }

        if found == false {
            return Ok(false);
        }
    }

    Ok(true)
}

fn handle_git_state(options: &Options, log: &mut Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let session = Git::get_session(&git_dir, log)?;

    if session.state == GitSessionState::CHERRYPICK {
        let log_read = log.clone();
        let next_hash = log_read.next_commit();

        // Check for empty commit
        if session.unmerged_paths.len() == 0 && session.modified_paths.len() == 0 {
            Git::cmd("cherry-pick --abort".to_string(), &git_dir)?;
            log.commit_update(next_hash, "empty")?;
            return Ok(());
        }

        // If we have conflicts then edit them
        if session.unmerged_paths.len() != 0 {
            cmd_edit(options, log)?;
            return Ok(());
        }

        // Check if all conflicts are resolved so we can update log and continue
        if session.unmerged_paths.len() == 0 && session.modified_paths.len() > 0 {
            Git::cmd("cherry-pick --continue".to_string(), &git_dir)?;
            let new_hash = Git::get_last_commit(&git_dir)?;
            log.commit_update(next_hash, &new_hash)?;
            return Ok(());
        }
    } else if session.state != GitSessionState::NONE {
        return Err("In session".into());
    }

    Ok(())
}

pub fn cmd_apply(options: &Options, log: &mut Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let branch = options.branch.clone().unwrap();
    let log_read = log.clone();
    let mut i: u32 = log_read.next_index();
    let num_commits = log_read.num_commits()?;

    Git::set_branch(&branch, &git_dir)?;

    let cherrypick_cache = get_cherrypick_cache(options)?;
    let commit_cache = get_commit_cache(options)?;

    // After this call the tree should be clean and ready to enter the apply loop
    handle_git_state(options, log)?;

    loop {
        let log_read = log.clone();
        let next_hash = log_read.next_commit();
        let commit = Git::show(&next_hash, &git_dir)?;

        println!("{} {}/{}: {} {}", "Applying".green(), i, num_commits, next_hash, commit.subject);
        i += 1;

        // Check for obvious cherry picks (commits WITH cherry pick tag) before trying to apply
        let mut is_cherrypick = false;
        for cherry in cherrypick_cache.iter() {
            let mut cherry_hash = "";

            if commit.hash == cherry.0 {
                cherry_hash = &cherry.1;
            } else if commit.hash == cherry.1 {
                cherry_hash = &cherry.0;
            }

            if cherry_hash != "" {
                println!("{} {}","Found cherry pick:".green(), cherry_hash);
                is_cherrypick = true;
                log.commit_update(next_hash, format!("cherry pick {}", cherry_hash).as_str())?;
                break;
            }
        }

        if is_cherrypick {
            continue;
        }

        // Apply commit
        let res = Git::cmd(format!("cherry-pick {} > /dev/null", next_hash), &git_dir);

        match res {
            Ok(_) => {
                let new_hash = Git::get_last_commit(&git_dir)?;
                log.commit_update(next_hash, &new_hash)?;
            },
            Err(_) => {
                // If apply fails, check for duplicates (commits WITHOUT cherry pick tag)
                let mut is_duplicate = false;

                for cache_item in commit_cache.iter() {
                    // Do a quick compare on subject to avoid the costly compare_patches() call.
                    if commit.subject == cache_item.1 {
                        let res = compare_patches(options, &commit.hash, &cache_item.0)?;
                        if res {
                            println!("{} {}", "Found duplicate:".yellow(), cache_item.0);
                            is_duplicate = true;
                            log.commit_update(next_hash, format!("duplicate {}", cache_item.0).as_str())?;
                            break;
                        }
                    }
                }

                if is_duplicate {
                    continue;
                }

                handle_git_state(options, log)?;

                let session = Git::get_session(&git_dir, log)?;

                // If the user didn't fix the conflict we abort
                if session.unmerged_paths.len() != 0 {
                    return Err("Conflict not resolved".into());
                }
            },
        }
    }
}

fn print_session(git_dir: &String, log: &Log) -> Result<(), Box<dyn Error>> {
    let session = Git::get_session(&git_dir, log)?;

    if session.modified_paths.len() > 0 {
        println!("\nChanges to be committed:");
        for path in session.modified_paths.iter() {
            println!("\t{}", path.1.green());
        }
    }

    if session.unmerged_paths.len() > 0 {
        println!("\nUnmerged paths:");
        for path in session.unmerged_paths.iter() {
            println!("\t{}", path.1.red());
        }
    }

    if session.unstaged_paths.len() > 0 {
        println!("\nChanges not staged for commit:");
        for path in session.unstaged_paths.iter() {
            println!("\t{}", path.1.red());
        }
    }

    Ok(())
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

pub fn cmd_edit(options: &Options, log: &mut Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let range_stop = options.range_stop.clone().unwrap();
    let session = Git::get_session(&git_dir, log)?;
    let commit = log.next_commit();

    print_session(&git_dir, log)?;

    for path in session.unmerged_paths.iter() {
        let file = &path.1;
        let file_path = Path::new(file);
        let commit_file = format!("/tmp/{}.patch", commit);
        let target_file = format!("/tmp/{}-{}", range_stop, file_path.file_name().unwrap().to_str().unwrap());

        loop {
            let ask = Util::ask(format!("Edit {} (Y)es/(n)o)/(s)kip commit/(a)bort? ", file.bold()), vec!["y", "n", "s", "a"], "y");
            let val = ask.as_str();

            match val {
                "n" => break,
                "a" => return Err("Aborted by user".into()),
                "s" => {
                    cmd_skip(options, log)?;
                    return Ok(());
                },
                _ => val,
            };

            // Find the line number where to start editing
            let lineno = find_conflict_lineno(format!("{}/{}", git_dir, file))?;

            // Store the commit as a patch file
            Git::cmd(format!("show {} > {}", commit, commit_file), &git_dir)?;

            // Store the target version of the file (eg git show v5.5:<filepath>)
            Git::cmd(format!("show {}:{} > {}", range_stop, file, target_file), &git_dir)?;

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

            // Check lineno again to see if all conflicts are solved
            let lineno = find_conflict_lineno(format!("{}/{}", git_dir, file))?;
            if lineno == "0" {
                Git::cmd(format!("add {file}"), &git_dir)?;
                break;
            } else {
                println!("{}", "File still contains conflics!".red());
            }
        }
        let session = Git::get_session(&git_dir, log)?;

        if session.unmerged_paths.len() == 0 && session.modified_paths.len() > 0 {
            println!("All conflicts resolved.");
        }
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
    let num_commits = log.num_commits()?;
    let percentage = (next_index / num_commits) * 100;
    println!("Progress {}% ({}/{})", percentage, next_index, num_commits);

    let stdout = Git::cmd(format!("diff --stat {branch} {range_stop} -- {paths}"), &git_dir)?;
    let lines: Vec<&str> = stdout.split("\n").collect();
    let summary = lines[lines.len() - 2].trim();

    println!("{summary}\n");

    let session = Git::get_session(&git_dir, log)?;

    match session.state {
        GitSessionState::CHERRYPICK => println!("{}", "Session: Cherry-picking".yellow()),
        GitSessionState::REBASE => println!("{}", "Session: Rebasing\n".yellow()),
        GitSessionState::NONE => println!("No session"),
    }

    print_session(&git_dir, log)?;

    let next_commit = log.next_commit();
    let commit = Git::show(next_commit, &git_dir)?;
    println!("\nNext commit to apply:\n{} {}", commit.hash, commit.subject);

    Ok(())
}

pub fn cmd_restart(options: &Options, log: &mut Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let branch_point = options.branch_point.clone().unwrap();

    let val = Util::ask("This operation will delete all current progress and restart applying commits from the beginning. Are you sure? (y)es/(N)o: ".to_string(), vec!["y", "n"], "n");

    if val != "y" {
        return Ok(());
    }

    println!("Reseting...");
    Git::cmd(format!("reset --hard {}", branch_point), &git_dir)?;

    let mut commits = String::from("");
    let lines: Vec<&str> = log.commits.split("\n").collect();

    for line in lines {
        let cols: Vec<&str> = line.trim().split(" ").collect();

        if cols.len() >= 2 && cols[0].len() == 40 {
            commits.push_str(cols[0]);
            commits.push_str("\n");
        } else {
            commits.push_str(line);
            commits.push_str("\n");
        }
    }

    log.commits = commits;
    log.save()?;
    Ok(())
}

pub fn cmd_skip(options: &Options, log: &mut Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let log_read = log.clone();

    let session = Git::get_session(&git_dir, log)?;

    if session.state == GitSessionState::CHERRYPICK {
        Git::cmd("cherry-pick --abort".to_string(), &git_dir)?;
    }

    let next_commit = log_read.next_commit();
    log.commit_update(next_commit, "skip")?;

    println!("Skipped {next_commit}.");

    Ok(())
}

pub fn cmd_diff(options: &Options) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let branch = options.branch.clone().unwrap();
    let range_stop = options.range_stop.clone().unwrap();
    let paths = options.paths.clone().unwrap();

    println!("{}", "Difference between current branch and range-stop".green());

    let stdout = Git::cmd(format!("diff {branch} {range_stop} -- {paths}").to_string(), &git_dir)?;
    println!("{stdout}");

    println!("{}", "------------------------------------------------------------".green());

    let stdout = Git::cmd(format!("diff --stat {branch} {range_stop} -- {paths}").to_string(), &git_dir)?;
    println!("{stdout}");

    Ok(())
}
