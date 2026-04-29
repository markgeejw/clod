mod argv;
mod cli;
mod paths;
mod profile;
mod run;
mod state;

use std::process::ExitCode;

use anyhow::Result;
use clap::{CommandFactory, Parser};

use crate::cli::{Cli, Command};
use crate::paths::Paths;

fn main() -> ExitCode {
    let known: Vec<String> = Cli::command()
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect();
    let argv = argv::rewrite(std::env::args_os().collect(), &known);
    let cli = Cli::parse_from(argv);
    match dispatch(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {:#}", e);
            ExitCode::from(1)
        }
    }
}

fn dispatch(cli: Cli) -> Result<()> {
    let paths = Paths::resolve(cli.clod_home, cli.claude_home)?;

    match cli.command {
        None => run::exec(&paths, &cli.claude_bin, &[]),
        Some(Command::Init) => {
            profile::init(&paths)?;
            println!("initialized {}", paths.clod_home.display());
            Ok(())
        }
        Some(Command::New { name }) => {
            profile::create(&paths, &name)?;
            println!(
                "created profile `{}` at {}",
                name,
                paths.profile_dir(&name).display()
            );
            Ok(())
        }
        Some(Command::Ls) => {
            let names = profile::list(&paths)?;
            let active = state::read_active(&paths)?.map(|a| a.name);
            if names.is_empty() {
                println!("(no profiles — try `clod new <name>`)");
                return Ok(());
            }
            for name in names {
                let mark = if Some(&name) == active.as_ref() {
                    "*"
                } else {
                    " "
                };
                println!("{} {}", mark, name);
            }
            Ok(())
        }
        Some(Command::Switch { name }) => {
            paths::validate_profile_name(&name)?;
            if !paths::exists(&paths.profile_dir(&name)) {
                anyhow::bail!(
                    "profile `{}` does not exist; create it with `clod new {}`",
                    name,
                    name
                );
            }
            state::write_active(&paths, &name)?;
            println!("switched to `{}`", name);
            Ok(())
        }
        Some(Command::Current) => {
            let active = state::resolve_active(&paths)?;
            let source = match active.source {
                state::ActiveSource::Env => "CLOD_PROFILE",
                state::ActiveSource::File => "active file",
            };
            println!("{} (from {})", active.name, source);
            Ok(())
        }
        Some(Command::Rm { name, yes }) => {
            profile::remove(&paths, &name, yes)?;
            println!("removed profile `{}`", name);
            Ok(())
        }
        Some(Command::Run { claude_args }) => run::exec(&paths, &cli.claude_bin, &claude_args),
        Some(Command::Completions { shell }) => {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            let mut out = std::io::stdout();
            clap_complete::generate(shell, &mut cmd, &bin_name, &mut out);
            // Augment fish output so `clod switch <TAB>` / `clod rm <TAB>`
            // suggest existing profile names by shelling out to `clod ls`.
            if matches!(shell, clap_complete::Shell::Fish) {
                use std::io::Write;
                writeln!(out, "{}", FISH_PROFILE_COMPLETIONS)?;
            }
            Ok(())
        }
    }
}

const FISH_PROFILE_COMPLETIONS: &str = r#"
# Dynamic profile-name completion for `clod switch` / `clod rm`.
function __clod_profile_names
    command clod ls 2>/dev/null | string replace -r '^[* ] ' ''
end
complete -c clod -n "__fish_clod_using_subcommand switch" -f -a "(__clod_profile_names)"
complete -c clod -n "__fish_clod_using_subcommand rm" -f -a "(__clod_profile_names)"
"#;
