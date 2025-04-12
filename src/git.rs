use std::process::Command;
use std::error::Error;

#[derive(Debug)]
pub struct Commit {
    pub hash: String,
    pub subject: String,
}

pub struct Git {
}

#[derive(Debug, PartialEq)]
pub enum GitSession {
    NONE,
    REBASE,
    CHERRYPICK,
}

impl Git {
    // Execute query in git repository located at dir
    pub fn cmd(query: String, dir: &String) -> Result<String, Box<dyn Error>> {

        let query = format!("git -C {} {}", dir, query);

        let output = Command::new("sh")
            .arg("-c")
            .arg(&query)
            .output()
            .expect(format!("Failed to execute: {}\n", &query).as_str());

        let stdout = String::from_utf8(output.stdout).expect("Invalid UTF8");

        if !output.status.success() {
            println!("{}", stdout);

            return Err(format!("Failed: {}", query).into());
        }

        Ok(stdout)
    }

    pub fn log(hash: &str, dir: &String) -> Result<Commit, Box<dyn Error>> {

        let mut commit = Commit {
            hash: "".to_string(),
            subject: "".to_string(),
        };

        let stdout: &str = &Git::cmd(format!("log --format='%H%n%s' -n1 {}", hash), dir)?;
        let lines: Vec<&str> = stdout.split("\n").collect();

        commit.hash = lines[0].trim().to_string();
        commit.subject = lines[1].trim().to_string();

        return Ok(commit);
    }

    pub fn get_last_commit(dir: &String) -> Result<String, Box<dyn Error>> {
        let res = Git::cmd(format!("log --format='%H' -n 1"), &dir);

        match res {
            Ok(commit) => return Ok(commit.trim().to_string()),
            Err(error) => return Err(error),
        };
    }

    pub fn get_branch(dir: &String) -> Result<String, Box<dyn Error>> {
        let stdout: &str = &Git::cmd("branch --show-current".to_string(), dir)?;
        let branch = stdout.to_string();

        Ok(branch)
    }

    pub fn set_branch(branch: &String, dir: &String) -> Result<(), Box<dyn Error>> {
        let current_branch = Git::get_branch(dir)?;

        if current_branch.trim() == *branch {
            return Ok(())
        }

        Git::cmd(format!("checkout {branch}"), dir)?;

        Ok(())
    }

    pub fn get_session(dir: &String) -> Result<GitSession, Box<dyn Error>> {
        let stdout: &str = &Git::cmd("status".to_string(), dir).unwrap();
        let lines: Vec<&str> = stdout.split("\n").collect();

        if lines[0].trim() == "interactive rebase in progress" {
            return Ok(GitSession::REBASE);
        } else if lines[1].len() > 39 && lines[1][..39].trim() == "You are currently cherry-picking commit" {
            return Ok(GitSession::CHERRYPICK);
        }

        Ok(GitSession::NONE)
    }

    pub fn parse_session_paths(dir: &String, typestr: &str) -> Result<Vec<(String, String)>, Box<dyn Error>> {
        let stdout: &str = &Git::cmd("status".to_string(), dir)?;
        let sections: Vec<&str> = stdout.split(typestr).collect();
        let mut paths: Vec<(String, String)> = Vec::new();

        if sections.len() != 2 {
            return Ok(paths);
        }

        let lines: Vec<&str> = sections[1].split("\n").collect();

        for line in lines.iter() {
            let col: Vec<&str> = line.split(":").collect();

            if col.len() == 2 {
                let path_type: String = col[0].trim().to_string();
                let path: String = col[1].trim().to_string();
                paths.push((path_type, path));
            }

            // Skip invalid lines while paths is empty, then break on first invalid line
            if col.len() == 1 && col[0] == ""  && paths.len() > 0 {
                break;
            }
        }

        Ok(paths)
    }

    pub fn get_unmerged_paths(dir: &String) -> Result<Vec<(String, String)>, Box<dyn Error>> {
        Git::parse_session_paths(&dir, "Unmerged paths:")
    }

    pub fn get_modified_paths(dir: &String) -> Result<Vec<(String, String)>, Box<dyn Error>> {
        Git::parse_session_paths(&dir, "Changes to be committed:")
    }

    pub fn get_unstaged_paths(dir: &String) -> Result<Vec<(String, String)>, Box<dyn Error>> {
        Git::parse_session_paths(&dir, "Changes not staged for commit:")
    }
}
