use clap::{command, Arg, ArgAction, Command};

pub struct Cli {
}

impl Cli {
    pub fn parse() -> Command {
        command!()
            .name("b2tf")
            .display_name("Back 2 The Future")
            .author("Patrik Jakobsson <patrik.r.jakobsson@gmail.com>")
            .arg_required_else_help(true)
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
            .subcommand(
                Command::new("setup")
                    .about("create b2tf.log file with supplied options")
            )
            .subcommand(
                Command::new("populate")
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
                Command::new("edit")
                    .about("edit the current conflict")
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
                Command::new("diffstat")
                    .about("show diff stat between your branch and <range stop>")
            )
            .subcommand(
                Command::new("rebase")
                    .about("rebase the commit list")
            )
            .subcommand(
                Command::new("prepend")
                    .about("add commit hash to beginning of log")
                    .arg_required_else_help(true)
                    .arg(Arg::new("hashes to prepend")
                        .long("hash")
                    )
            )
            .subcommand(
                Command::new("append")
                    .about("add commit hash to end of log")
                    .arg_required_else_help(true)
                    .arg(Arg::new("hashes to append")
                        .long("hash")
                    )
            )
            .subcommand(
                Command::new("insert")
                    .about("insert hashes before or after specified hash")
                    .arg_required_else_help(true)
                    .arg(Arg::new("hashes to append")
                        .long("hash")
                    )
                    .arg(Arg::new("insert after this hash")
                        .long("after")
                    )
            )
            .subcommand(
                Command::new("suse")
                    .about("SUSE specific subcommands")
                    .arg_required_else_help(true)
                    .arg(Arg::new("signature")
                        .long("signature")
                    )
                    .arg(Arg::new("patch references")
                        .long("references")
                    )
                    .arg(Arg::new("Path to SUSE kernel-source")
                        .long("suse-kernel-source")
                    )
                    .subcommand(
                        Command::new("export")
                            .about("export all commits as SUSE patch files")
                    )
                    .subcommand(
                        Command::new("unblacklist")
                            .about("remove blacklists for patches we are backporting")
                    )
                    .subcommand(
                        Command::new("apply")
                            .about("apply all patches to the SUSE tree")
                    )
            )
    }
}
