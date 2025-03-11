use std::process::Command;
use std::error::Error;
use crate::Options;
use crate::Log;

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
