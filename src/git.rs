use std::process::Command;
use std::error::Error;

#[derive(Debug)]
pub struct Git {
    pub dir:    Option<String>,
}

impl Git {
    pub fn cmd(query: String, dir: &String) -> Result<(), Box<dyn Error>> {

        let query = format!("git -C {} {}", dir, query);

        let output = Command::new("sh")
            .arg("-c")
            .arg(&query)
            .output()
            .expect(format!("Failed to execute: {}\n", &query).as_str());

        if !output.status.success() {
            return Err(format!("git cmd failed. Query: git -C {} {}", dir, query).into());
        } else {
            println!("{}", query);
        }

        let stdout = String::from_utf8(output.stdout).expect("Invalid UTF8");
        println!("{}", stdout);

        Ok(())
    }
}
