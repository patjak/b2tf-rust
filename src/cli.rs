use std::fmt::Debug;
use clap::{command, Arg, ArgAction, ArgMatches, Command};

#[derive(Debug)]
pub struct Options {
    pub range_start:    Option<String>,
    pub range_stop:     Option<String>,
    pub branch:         Option<String>,
    pub branch_point:   Option<String>,
    pub work_dir:       Option<String>,
    pub git_dir:        Option<String>,
    pub paths:          Option<String>,
    pub signature:      Option<String>,
    pub references:     Option<String>,
}

impl Options {
    pub fn parse_matches(&mut self, matches: ArgMatches) {

        self.range_start = matches.get_one::<String>("first commit").cloned();
        self.range_stop = matches.get_one::<String>("last commit").cloned();
        self.branch = matches.get_one::<String>("branch name").cloned();
        self.branch_point = matches.get_one::<String>("branch point").cloned();
        self.work_dir = matches.get_one::<String>("work directory").cloned();
        self.git_dir = matches.get_one::<String>("git directory").cloned();
        self.paths = matches.get_one::<String>("paths").cloned();
        self.signature = matches.get_one::<String>("signature").cloned();
        self.references = matches.get_one::<String>("patch references").cloned();
    }
}

pub struct Cli {
}

impl Cli {
    pub fn parse_command_line() -> Options {
        // Start by looking at the command line
        let matches = command!()
            .arg(Arg::new("debug")
                .long("debug")
                .action(ArgAction::SetTrue)
            )
            .arg(Arg::new("first commit")
                .long("range-start")
            )
            .arg(Arg::new("last commit")
                .long("range-stop")
            )
            .arg(Arg::new("branch name")
                .long("branch")
            )
            .arg(Arg::new("branch point")
                .long("branch-point")
            )
            .arg(Arg::new("work directory")
                .long("work-dir")
            )
            .arg(Arg::new("git directory")
                .long("git-dir")
            )
            .arg(Arg::new("paths")
                .long("paths")
            )
            .arg(Arg::new("signature")
                .long("signature")
            )
            .arg(Arg::new("patch references")
                .long("references")
            )

            .subcommand(
                Command::new("export")
                    .about("populate the commits list from commits inside range")
            )
            .subcommand(
                Command::new("apply")
                    .about("apply patches from the commit list into your branch")
            )
            .subcommand(
                Command::new("skip")
                    .about("skip current commit")
            )
            .subcommand(
                Command::new("restart")
                    .about("delete your branch and restart the entire backport")
            )
            .subcommand(
                Command::new("status")
                    .about("show status of backport")
            )
            .subcommand(
                Command::new("diff")
                    .about("show diff between your branch and <range stop>")
            )
            .subcommand(
                Command::new("diffdiff")
                    .about("show diff between your branch and <range stop> without diff from branch and <range start>")
            )
            .subcommand(
                Command::new("rebase")
                    .about("rebase the commit list")
            )
            .get_matches();

        println!("debug is {}", matches.get_flag("debug"));

        let mut options = Options {
            range_start: None,
            range_stop:  None,
            branch: None,
            branch_point: None,
            work_dir: None,
            git_dir: None,
            paths: None,
            signature: None,
            references: None,
        };
        options.parse_matches(matches);

        // Then check the local config file

        // Then check the users global config file

        options
    }
}
