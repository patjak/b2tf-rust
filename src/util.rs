use std::io::{stdin,stdout,Write};

pub struct Util {
}

impl Util {
    pub fn ask(msg: String, opts: Vec<&str>, default: &str) -> String {
        print!("{msg}");
        let mut val = String::new();
        let _= stdout().flush();

        stdin().read_line(&mut val).expect("Invalid input");
        if let Some('\n') = val.chars().next_back() {
            val.pop();
        }
        if let Some('\r') = val.chars().next_back() {
            val.pop();
        }

        if val == "" {
            return default.to_string();
        }

        if !opts.contains(&val.as_str()) {
            Util::ask(msg, opts, default);
        }

        val
    }
}
