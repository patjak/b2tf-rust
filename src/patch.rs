extern crate unidiff;
use std::error::Error;
use colored::Colorize;
use unidiff::{PatchSet, PatchedFile, Hunk, Line};

#[derive(Debug)]
pub struct PatchLine {
    pub source_line_no: Option<usize>,
    pub target_line_no: Option<usize>,
    pub diff_line_no: usize,
    pub line_type: String,
    pub value: String,
}

impl PatchLine {
    pub fn new() -> Self {
        Self {
            source_line_no: None,
            target_line_no: None,
            diff_line_no: 0,
            line_type: String::new(),
            value: String::new(),
        }
    }

    pub fn print(&self) {
        if self.line_type == "+" {
            println!("{} {}", self.line_type.green(), self.value.green());
        } else if self.line_type == "-" {
            println!("{} {}", self.line_type.red(), self.value.red());
        } else {
            println!("{} {}", self.line_type, self.value);
        }
    }

    pub fn parse(&mut self, line: &Line) {
        self.source_line_no = line.source_line_no;
        self.target_line_no = line.target_line_no;
        self.diff_line_no = line.diff_line_no;
        self.line_type = line.line_type.clone();
        self.value = line.value.clone();
    }

    pub fn compare(&self, line: &PatchLine, fuzz: bool) -> bool {
        if self.value == line.value && self.line_type == line.line_type {
            if fuzz {
                return true;
            }
            if self.source_line_no == line.source_line_no &&
               self.target_line_no == line.target_line_no {
                return true;
            }
        }
        false
    }
}

#[derive(Debug)]
pub struct PatchHunk {
    pub source_start: usize,
    pub source_length: usize,
    pub target_start: usize,
    pub target_length: usize,
    pub section_header: String,
    pub lines: Vec<PatchLine>,
}

impl PatchHunk {
    pub fn new() -> Self {
        Self {
            source_start: 0,
            source_length: 0,
            target_start: 0,
            target_length: 0,
            section_header: String::new(),
            lines: vec![],
        }
    }

    pub fn print(&self) {
        print!("{}", format!("@@ -{},{} ", self.source_start, self.source_length).cyan());
        print!("{}", format!("+{},{} @@ ", self.target_start, self.target_length).cyan());
        println!("{}", self.section_header);
        for line in &self.lines {
            line.print();
        }
    }

    pub fn parse(&mut self, hunk: &Hunk) {
        self.source_start = hunk.source_start;
        self.source_length = hunk.source_length;
        self.target_start = hunk.target_start;
        self.target_length = hunk.target_length;
        self.section_header = hunk.section_header.clone();

        for l1 in hunk.lines() {
            let mut l2 = PatchLine::new();
            l2.parse(l1);
            self.lines.push(l2);
        }
    }

    pub fn compare(&self, hunk: &PatchHunk, fuzz: bool) -> bool {
        if self.section_header != hunk.section_header {
            return false;
        }

        if fuzz {
            if self.source_start != hunk.source_start ||
               self.source_length != hunk.source_length ||
               self.target_start != hunk.target_start ||
               self.target_length != hunk.target_length {
                   return false;
               }
        }

        if self.lines.len() != hunk.lines.len() {
            return false;
        }

        for i in 0..self.lines.len() {
            let l1 = &self.lines[i];
            let l2 = &hunk.lines[i];

            if !l1.compare(l2, fuzz) {
                return false;
            }
        }
        true
    }

}

#[derive(Debug)]
pub struct PatchFile {
    pub source_file: String,
    pub target_file: String,
    pub hunks: Vec<PatchHunk>,
}

impl PatchFile {
    pub fn new() -> Self {
        Self {
            source_file: String::new(),
            target_file: String::new(),
            hunks: vec![],
        }
    }

    pub fn print(&self) {
        println!("--- {}", self.source_file);
        println!("+++ {}", self.target_file);

        for hunk in &self.hunks {
            hunk.print();
        }
    }

    pub fn parse(&mut self, file: PatchedFile) {
        self.source_file = file.source_file.clone();
        self.target_file = file.target_file.clone();

        for h1 in file.hunks() {
            let mut h2 = PatchHunk::new();

            h2.parse(h1);
            self.hunks.push(h2);
        }
    }

    pub fn compare(&self, file: &PatchFile, fuzz: bool) -> bool {
        if self.source_file != file.source_file ||
           self.target_file != file.target_file {
               return false;
        }

        if self.hunks.len() != file.hunks.len() {
            return false;
        }

        for i in 0..self.hunks.len() {
            let h1 = &self.hunks[i];
            let h2 = &file.hunks[i];
            if !h1.compare(h2, fuzz) {
                return false;
            }
        }
        true
    }
}

#[derive(Debug)]
pub struct Patch {
    files: Vec<PatchFile>,
}

impl Patch {
    pub fn new() -> Self {
        Self {
            files: vec![],
        }
    }

    pub fn print(&self) {
        for file in &self.files {
            file.print();
        }
    }

    pub fn parse(&mut self, diff: &String) -> Result<(), Box<dyn Error>> {
        let mut unidiff = PatchSet::new();
        unidiff.parse(diff).expect("Error parsing diff with unidiff");

        let mut patch = Patch::new();

        for file in unidiff {
            let mut f = PatchFile::new();
            f.parse(file);
            self.files.push(f);
        }
        Ok(())
    }
}
