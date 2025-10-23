extern crate unidiff;
use std::process::Command;
use std::error::Error;
use std::path::Path;
use std::fs;
use std::io::Write;
use clap::ArgMatches;
use colored::Colorize;
use crate::Options;
use crate::Log;
use crate::Util;
use crate::git::{Git, GitSessionState};
use unidiff::PatchSet;
use mktemp::Temp;

pub fn cmd_setup(matches: &ArgMatches) -> Result<(), Box<dyn Error>> {
    let mut options = Options::new();
    let mut log = Log::new();

    if fs::exists(&log.filename)? {
        return Err("b2tf.log already exists. Will not overwrite an existing configuration.".into());
    }

    options.parse_matches(matches);

    if options.range_start.is_some() {
        log.config.push_str(format!("range-start: {}\n", &options.range_start.unwrap()).as_str());
    } else {
        return Err("--range-start must be specified".into());
    }
    if options.range_stop.is_some() {
        log.config.push_str(format!("range-stop: {}\n", &options.range_stop.unwrap()).as_str());
    } else {
        return Err("--range-stop must be specified".into());
    }
    if options.branch.is_some() {
        log.config.push_str(format!("branch: {}\n", &options.branch.unwrap()).as_str());
    } else {
        return Err("--branch must be specified".into());
    }
    if options.branch_point.is_some() {
        log.config.push_str(format!("branch-point: {}\n", &options.branch_point.unwrap()).as_str());
    }
    if options.work_dir.is_some() {
        log.config.push_str(format!("work-dir: {}\n", &options.work_dir.unwrap()).as_str());
    }
    if options.git_dir.is_some() {
        log.config.push_str(format!("git-dir: {}\n", &options.git_dir.unwrap()).as_str());
    } else {
        return Err("--git-dir must be specified.".into());
    }
    if options.paths.is_some() {
        log.config.push_str(format!("paths: {}\n", &options.paths.unwrap()).as_str());
    }
    if options.signature.is_some() {
        log.config.push_str(format!("signature: {}\n", &options.signature.unwrap()).as_str());
    }
    if options.references.is_some() {
        log.config.push_str(format!("references: {}\n", &options.references.unwrap()).as_str());
    }

    log.save()
}

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

        let subject = format!("# {}\n", &line[41..]);
        let hash = format!("{}\n\n", &line[0..40]);

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

/* Describes how well two patches match eachother
 *
 * Different    - no match at all
 * Similar      - the changes are the same but with different line numbers
 * Same         - changes and line number are the same but patch files are not identical
 * Identical    - the patches contain exactly the same contents
 */
#[derive(PartialEq, PartialOrd)]
pub enum CompareResult {
    Different = 0,
    Similar = 1,
    Same = 2,
    Identical = 3,
}

fn compare_diffs(diff1: &String, diff2: &String) -> Result<CompareResult, Box<dyn Error>> {

    // Compare strings directly
    if diff1 == diff2 {
        return Ok(CompareResult::Identical);
    }

    let mut patch1 = PatchSet::new();
    patch1.parse(diff1).expect("Error parsing diff");

    let mut patch2 = PatchSet::new();
    patch2.parse(diff2).expect("Error parsing diff");

    if patch1.len() != patch2.len() {
        return Ok(CompareResult::Different);
    }

    let files1 = patch1.files();
    let files2 = patch2.files();

    // Stores whether line numbers are not matching
    let mut context_mismatch = false;

    for i in 0..(files1.len() - 1) {
        let file1 = &files1[i];
        let file2 = &files2[i];

        if file1.len() != file2.len() {
            return Ok(CompareResult::Different);
        }

        let hunks1 = file1.hunks();
        let hunks2 = file2.hunks();

        if hunks1.len() != hunks2.len() {
            return Ok(CompareResult::Different);
        }

        for j in 0..(hunks1.len() - 1) {
            if hunks1[j].section_header != hunks2[j].section_header {
                return Ok(CompareResult::Different);
            }

            let lines1 = hunks1[j].lines();
            let lines2 = hunks2[j].lines();

            if lines1.len() != lines2.len() {
                return Ok(CompareResult::Different);
            }

            for k in 0..lines1.len() {
                let line1 = &lines1[k];
                let line2 = &lines2[k];

                if line1.source_line_no != line2.source_line_no ||
                   line1.target_line_no != line2.target_line_no ||
                   line1.diff_line_no != line2.diff_line_no {
                       context_mismatch = true;
                }

                if line1.line_type != line2.line_type ||
                   line1.value != line2.value {
                       return Ok(CompareResult::Different);
                }
            }
        }
    }

    if context_mismatch {
        return Ok(CompareResult::Similar);
    }

    Ok(CompareResult::Same)
}

pub fn compare_patches(src_path: &str, dst_path: &str) -> Result<CompareResult, Box<dyn Error>> {
    let src: String = fs::read_to_string(src_path)?;
    let dst: String = fs::read_to_string(dst_path)?;

    Ok(compare_diffs(&src, &dst)?)
}

fn compare_commits(options: &Options, hash1: &str, hash2: &str) -> Result<CompareResult, Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();

    let commit1 = Git::show(hash1, &git_dir)?;
    let commit2 = Git::show(hash2, &git_dir)?;

    Ok(compare_diffs(&commit1.body, &commit2.body)?)
}

// Returns true if a patch was applied
fn handle_git_state(options: &Options, log: &mut Log) -> Result<bool, Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let session = Git::get_session(&git_dir)?;

    if session.state == GitSessionState::Cherrypick {
        let log_read = log.clone();
        let next_hash = log_read.next_commit();

        // Check for empty commit
        if session.unmerged_paths.is_empty() && session.modified_paths.is_empty() {
            Git::cmd("cherry-pick --abort".to_string(), &git_dir)?;
            log.commit_update(next_hash, "empty")?;
            println!("{} {}", "Empty commit:".bright_blue(), next_hash.bright_blue());
            return Ok(true);
        }

        // If we have conflicts then edit them
        if !session.unmerged_paths.is_empty() {
            let handled = cmd_edit(options, log)?;
            return Ok(handled);
        }

        // Check if all conflicts are resolved so we can update log and continue
        if session.unmerged_paths.is_empty() && !session.modified_paths.is_empty() {
            Git::cmd("cherry-pick --continue".to_string(), &git_dir)?;
            let new_hash = Git::get_last_commit(&git_dir)?;
            log.commit_update(next_hash, &new_hash)?;
            return Ok(true);
        }
    } else if session.state == GitSessionState::Rebase {
        if session.unmerged_paths.is_empty() && !session.modified_paths.is_empty() {
            Git::cmd_passthru("rebase --continue".to_string(), &git_dir)?;
        }

        // If we have conflicts then edit them
        if !session.unmerged_paths.is_empty() {
            cmd_edit(options, log)?;
            return Ok(false);
        }
    } else if session.state != GitSessionState::None {
        return Err("In session".into());
    }

    Ok(false)
}

pub fn cmd_apply(options: &Options, log: &mut Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let log_read = log.clone();
    let mut i: u32 = log_read.next_index();
    let num_commits = log_read.num_commits()?;

    let cherrypick_cache = get_cherrypick_cache(options)?;
    let commit_cache = get_commit_cache(options)?;

    // After this call the tree should be clean and ready to enter the apply loop
    handle_git_state(options, log)?;

    loop {
        let log_read = log.clone();
        let next_hash = log_read.next_commit();
        let commit = Git::show(next_hash, &git_dir)?;

        if next_hash.is_empty() {
            break Ok(());
        }

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

            if !cherry_hash.is_empty() {
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
                    // Do a quick compare on subject to avoid the costly compare_commits() call.
                    if commit.subject == cache_item.1 {
                        let res = compare_commits(options, &commit.hash, &cache_item.0)?;
                        if res >= CompareResult::Similar {
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

                if !handle_git_state(options, log)? {
                    i -= 1;
                }

                let session = Git::get_session(&git_dir)?;

                // If the user didn't fix the conflict we abort
                if !session.unmerged_paths.is_empty() {
                    return Err("Conflict not resolved".into());
                }
            },
        }
    }
}

fn print_session(git_dir: &String) -> Result<(), Box<dyn Error>> {
    let session = Git::get_session(git_dir)?;

    if !session.modified_paths.is_empty() {
        println!("\nChanges to be committed:");
        for path in session.modified_paths.iter() {
            println!("\t{}", path.1.green());
        }
    }

    if !session.unmerged_paths.is_empty() {
        println!("\nUnmerged paths:");
        for path in session.unmerged_paths.iter() {
            println!("\t{}", path.1.red());
        }
    }

    if !session.unstaged_paths.is_empty() {
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

pub fn cmd_edit(options: &Options, log: &mut Log) -> Result<bool, Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let range_stop = options.range_stop.clone().unwrap();
    let session = Git::get_session(&git_dir)?;
    let commit = log.next_commit();

    print_session(&git_dir)?;

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
                    return Ok(true);
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
        let session = Git::get_session(&git_dir)?;

        if session.unmerged_paths.is_empty() && !session.modified_paths.is_empty() {
            println!("All conflicts resolved.");
        }
    }

    Ok(false)
}

pub fn cmd_status(options: &Options, log: &Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let branch = options.branch.clone().unwrap();
    let paths = options.paths.clone().unwrap();
    let range_stop = options.range_stop.clone().unwrap();

    let next_index = log.next_index();
    let num_commits = log.num_commits()?;
    let percentage: f32 = ((next_index as f32) / (num_commits as f32)) * 100.0;
    println!("Progress {:.0}% ({}/{})", percentage, next_index, num_commits);

    let stdout = Git::cmd(format!("diff --stat {branch} {range_stop} -- {paths}"), &git_dir)?;
    let lines: Vec<&str> = stdout.split("\n").collect();
    let summary = lines[lines.len() - 2].trim();

    println!("{summary}\n");

    let session = Git::get_session(&git_dir)?;

    match session.state {
        GitSessionState::Cherrypick => println!("{}", "Session: Cherry-picking".yellow()),
        GitSessionState::Rebase => println!("{}", "Session: Rebasing\n".yellow()),
        GitSessionState::None => println!("No session"),
    }

    print_session(&git_dir)?;

    if next_index < num_commits {
        let next_commit = log.next_commit();
        let commit = Git::show(next_commit, &git_dir)?;
        println!("\nNext commit to apply:\n{} {}", commit.hash, commit.subject);
    }

    Ok(())
}

pub fn cmd_restart(options: &Options, log: &mut Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let branch_point = options.branch_point.clone().unwrap();

    let val = Util::ask("This operation will delete all current progress and restart applying commits from the beginning of the log.\nAre you sure? (y)es/(N)o: ".to_string(), vec!["y", "n"], "n");

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
            commits.push('\n');
        } else {
            commits.push_str(line);
            commits.push('\n');
        }
    }

    log.commits = commits;
    log.save()?;
    Ok(())
}

pub fn cmd_skip(options: &Options, log: &mut Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let log_read = log.clone();

    let session = Git::get_session(&git_dir)?;

    if session.state == GitSessionState::Cherrypick {
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

    let stdout = Git::cmd(format!("diff {branch} {range_stop} -- {paths}").to_string(), &git_dir)?;
    let mut patch = PatchSet::new();
    patch.parse(stdout).expect("Error parsing diff");

    for file in patch {
        println!("--- {}", file.source_file);
        println!("+++ {}", file.target_file);
        for hunk in file {
            print!("{}", format!("@@ -{},{} ", hunk.source_start, hunk.source_length).cyan());
            print!("{}", format!("+{},{} @@ ", hunk.target_start, hunk.target_length).cyan());
            println!("{}", hunk.section_header);
            for line in hunk {
                if line.line_type == "+" {
                    println!("{} {}", line.line_type.green(), line.value.green());
                } else if line.line_type == "-" {
                    println!("{} {}", line.line_type.red(), line.value.red());
                } else {
                    println!("{} {}", line.line_type, line.value);
                }
            }
        }
    }

    Ok(())
}

pub fn cmd_diffdiff(options: &Options) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let branch = options.branch.clone().unwrap();
    let range_start = options.range_start.clone().unwrap();
    let range_stop = options.range_stop.clone().unwrap();
    let paths = options.paths.clone().unwrap();

    let diff_start = Git::cmd(format!("diff {branch} {range_start} -- {paths}").to_string(), &git_dir)?;
    let diff_stop = Git::cmd(format!("diff {branch} {range_stop} -- {paths}").to_string(), &git_dir)?;

    let mut patch_start = PatchSet::new();
    patch_start.parse(diff_start).expect("Error parsing start diff");

    let mut patch_stop = PatchSet::new();
    patch_stop.parse(diff_stop).expect("Error parsing stop diff");

    let mut files = "".to_string();

    for file_stop in patch_stop {
        let mut hunks = "".to_string();

        for file_start in patch_start.clone() {
            if file_stop.source_file != file_start.source_file ||
               file_stop.target_file != file_start.target_file {
                continue;
            }

            for hunk_stop in file_stop.clone() {
                let mut lines = "".to_string();

                for hunk_start in file_start.clone() {
                    if hunk_stop.section_header != hunk_start.section_header {
                        continue;
                    }

                    for line_stop in hunk_stop.clone() {
                        let mut found = false;
                        for line_start in hunk_start.clone() {
                            if line_stop.line_type == line_start.line_type &&
                               line_stop.value == line_start.value {
                                   found = true;
                                   break;
                             }
                        }
                        if !found {
                            lines.push_str(format!("{} {}\n", line_stop.line_type, line_stop.value).as_str());
                        }
                    }

                }
                if !lines.is_empty() {
                    hunks.push_str(format!("@@ -{},{} +{},{} @@ {}\n", hunk_stop.source_start, hunk_stop.source_length,
                                                                     hunk_stop.target_start, hunk_stop.target_length,
                                                                     hunk_stop.section_header).as_str());
                    hunks.push_str(lines.as_str());
                }
            }
        }

        if !hunks.is_empty() {
            files.push_str(format!("--- {}\n", file_stop.source_file).as_str());
            files.push_str(format!("+++ {}\n", file_stop.target_file).as_str());
            files.push_str(hunks.as_str());
        }
    }

    println!("{}", files);

    Ok(())
}

pub fn cmd_diffstat(options: &Options) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let branch = options.branch.clone().unwrap();
    let range_stop = options.range_stop.clone().unwrap();
    let paths = options.paths.clone().unwrap();

    let stdout = Git::cmd(format!("diff --stat {branch} {range_stop} -- {paths}").to_string(), &git_dir)?;

    println!("{stdout}");

    Ok(())
}

pub fn cmd_rebase(options: &Options, log: &mut Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let range_start = options.range_start.clone().unwrap();
    let commits = log.get_all()?;
    let last_commit = log.last_applied_commit()?;
    let temp_file = Temp::new_file()?;
    let pathbuf = temp_file.to_path_buf();
    let filename = pathbuf.as_os_str().to_str().unwrap();
    let mut file = fs::File::create(&temp_file)?;
    let session = Git::get_session(&git_dir)?;

    if session.state == GitSessionState::None {
        for hash in commits {
            let mut picked_hash = &hash.0;

            if hash.1.len() == 40 {
                // If the commit is already backported we pick that hash
                picked_hash = &hash.1;
            } else if hash.1.len() > 0 {
                // Skip all empty/duplicates/cherry-picks etc.
                continue;
            }

            file.write_all(format!("pick {}\n", picked_hash).as_bytes())?;

            if hash.0 == last_commit {
                break;
            }
        }

        let query = format!("GIT_SEQUENCE_EDITOR='cp {} ' git -C {} rebase -i {}", filename, git_dir, range_start);

        Command::new("sh")
            .arg("-c")
            .arg(&query)
            .status()
            .expect(format!("Failed to execute: {}\n", &query).as_str());
    }

    if session.state != GitSessionState::Rebase && session.state != GitSessionState::None {
        return Err("Invalid session state.".into());
    }

    loop {
        let session = Git::get_session(&git_dir)?;
        if session.state == GitSessionState::None {
            break;
        }
        handle_git_state(options, log)?;
    }

    // Rebase succeeded. Now rebuild the commit list
    println!("");
    let stdout = Git::cmd(format!("log --oneline --reverse --format='%H %s' {}..", range_start), &git_dir)?;
    let lines: Vec<&str> = stdout.split("\n").collect();
    let commits = log.get_all()?;

    let mut j: usize = 0;
    for i in 0..(commits.len() - 1) {
        // Skip everything that is not a backported commit
        if commits[i].0.len() != 40 || commits[i].1.len() != 40 {
            continue;
        }

        let commit = Git::show(&commits[i].0, &git_dir)?;
        let hash_up = commit.hash;
        let subject_up = commit.subject;

        let mut cols: Vec<&str> = lines[j].split(" ").collect();

        let hash_down = &cols.remove(0);
        let subject_down = cols.join(" ");

        if subject_down != subject_up {
            return Err(format!("Log is out of sync with git repository at:\nGit: {} {}\nLog: {} {}",
                               hash_up, subject_up, hash_down, subject_down).into());
        }

        j += 1;
        print!("\rUpdating log: {}/{}", j, lines.len() - 1);
        log.commit_update(&hash_up, &hash_down)?;
    }

    println!("\nRebase done");

    Ok(())
}

pub fn cmd_prepend(options: &Options, log: &mut Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let hash_arg;

    match &options.hash {
        Some(arg) => hash_arg = arg,
        None => return Err("No --hash was provided".into()),
    };

    // Create a string to prepend to the commits log
    let mut prepend = String::new();

    let hashes: Vec<&str> = hash_arg.split(" ").collect();
    for hash in hashes {
        let commit = Git::show(hash, &git_dir)?;
        prepend.push_str(format!("# {}\n{}\n\n", commit.subject, commit.hash).as_str());
    }

    log.commits.insert_str(0, prepend.as_str());
    log.save()?;

    println!("Commits prepended:\n");
    print!("{}", prepend);

    Ok(())
}

pub fn cmd_append(options: &Options, log: &mut Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let hash_arg;

    match &options.hash {
        Some(arg) => hash_arg = arg,
        None => return Err("No --hash was provided".into()),
    };

    // Create a string to append to the commits log
    let mut append = String::new();

    let hashes: Vec<&str> = hash_arg.split(" ").collect();
    for hash in hashes {
        let commit = Git::show(hash, &git_dir)?;
        append.push_str(format!("# {}\n{}\n\n", commit.subject, commit.hash).as_str());
    }

    log.commits.push_str(append.as_str());
    log.save()?;

    println!("Commits appended:\n");
    print!("{}", append);

    Ok(())
}

pub fn cmd_insert(options: &Options, log: &mut Log) -> Result<(), Box<dyn Error>> {
    let git_dir = options.git_dir.clone().unwrap();
    let hash_arg;
    let mut after = String::new();

    match &options.hash {
        Some(arg) => hash_arg = arg,
        None => return Err("No --hash was provided".into()),
    };

    match &options.after {
        Some(arg) => after = arg.to_string(),
        None => (),
    };

    if after.is_empty() {
        return Err("Argument --after must be specified".into());
    }

    let mut insert = String::new();

    let hashes: Vec<&str> = hash_arg.split(" ").collect();
    for hash in hashes {
        let commit = Git::show(hash, &git_dir)?;
        insert.push_str(format!("# {}\n{}\n\n", commit.subject, commit.hash).as_str());
    }

    let mut commits = String::new();

    let lines: Vec<&str> = log.commits.split("\n").collect();
    for line in lines {
        let line = line.trim();

        let cols: Vec<&str> = line.split(" ").collect();
        if (cols.len() == 1 && cols[0] == after) ||
           (cols.len() == 2 && (cols[0] == after || cols[1] == after)) {
                commits.push_str(&line);
                commits.push_str("\n\n");
                commits.push_str(insert.as_str()); // Insert after
            continue;
        }
        commits.push_str(&line);
        commits.push_str("\n");
    }

    log.commits = commits;
    log.save()?;

    println!("Inserted the following commits:\n{}", insert);

    Ok(())
}
