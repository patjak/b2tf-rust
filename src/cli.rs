use clap::{command, Arg, ArgAction, ArgMatches, Command};

pub struct Cli {
}

impl Cli {
    pub fn parse() -> ArgMatches {
        let matches = command!()
            .name("Back 2 The Future")
            .display_name("display name")
            .author("Patrik Jakobsson <patrik.r.jakobsson@gmail.com>")
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

        matches
    }
}
