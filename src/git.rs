use std::process::Command;
use std::error::Error;
use crate::Log;

#[derive(Debug)]
pub struct Commit {
    pub hash: String,
    pub subject: String,
    pub body: String,
}

pub struct Git {
}

pub struct GitSession {
    pub state: GitSessionState,
    pub modified_paths: Vec<(String, String)>,
    pub unmerged_paths: Vec<(String, String)>,
    pub unstaged_paths: Vec<(String, String)>,
}

#[derive(Debug, PartialEq)]
pub enum GitSessionState {
    None,
    Rebase,
    Cherrypick,
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
            if !stdout.is_empty() {
                println!("{}", stdout);
            }

            return Err(format!("Failed: {}", query).into());
        }

        Ok(stdout)
    }

    pub fn show(hash: &str, dir: &String) -> Result<Commit, Box<dyn Error>> {

        let mut commit = Commit {
            hash: "".to_string(),
            subject: "".to_string(),
            body: "".to_string(),
        };

        let stdout: &str = &Git::cmd(format!("show --format='%H%n%s%n%b' -n1 {}", hash), dir)?;
        let lines: Vec<&str> = stdout.split("\n").collect();

        commit.hash = lines[0].trim().to_string();
        commit.subject = lines[1].trim().to_string();

        for line in lines.iter().skip(2) {
            commit.body.push_str(line);
            commit.body.push('\n');
        }

        Ok(commit)
    }

    pub fn get_last_commit(dir: &String) -> Result<String, Box<dyn Error>> {
        let res = Git::cmd("log --format='%H' -n 1".to_string(), dir);

        match res {
            Ok(commit) => Ok(commit.trim().to_string()),
            Err(error) => Err(error),
        }
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

    pub fn get_session(dir: &String, log: &Log) -> Result<GitSession, Box<dyn Error>> {
        let stdout: &str = &Git::cmd("status".to_string(), dir)?;

        let mut session: GitSession = GitSession {
            state:  GitSessionState::None,
            unmerged_paths: Git::get_unmerged_paths(stdout)?,
            modified_paths: Git::get_modified_paths(stdout)?,
            unstaged_paths: Git::get_unstaged_paths(stdout)?,
        };

        let lines: Vec<&str> = stdout.split("\n").collect();
        if lines[0].trim() == "interactive rebase in progress" {
            session.state  = GitSessionState::Rebase;
        } else if lines[1].len() > 39 && lines[1][..39].trim() == "You are currently cherry-picking commit" {
            session.state = GitSessionState::Cherrypick;
        }

        if session.state == GitSessionState::Cherrypick {
            let hash: Vec<&str> = stdout.split("You are currently cherry-picking commit ").collect();
            let hash: Vec<&str> = hash[1].split(".").collect();
            let hash = hash[0];
            let commit = Git::show(hash, dir)?;
            let next_hash = log.next_commit();

            if commit.hash != next_hash {
                return Err("GIT repository is out of sync with Log".into());
            }
        }

        Ok(session)
    }

    pub fn parse_session_paths(stdout: &str, typestr: &str) -> Result<Vec<(String, String)>, Box<dyn Error>> {
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
            if col.len() == 1 && col[0].is_empty()  && !paths.is_empty() {
                break;
            }
        }

        Ok(paths)
    }

    pub fn get_unmerged_paths(stdout: &str) -> Result<Vec<(String, String)>, Box<dyn Error>> {
        Git::parse_session_paths(stdout, "Unmerged paths:")
    }

    pub fn get_modified_paths(stdout: &str) -> Result<Vec<(String, String)>, Box<dyn Error>> {
        Git::parse_session_paths(stdout, "Changes to be committed:")
    }

    pub fn get_unstaged_paths(stdout: &str) -> Result<Vec<(String, String)>, Box<dyn Error>> {
        Git::parse_session_paths(stdout, "Changes not staged for commit:")
    }
}
