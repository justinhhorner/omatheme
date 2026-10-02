//! The command-line interface: arguments, output and the commands. Logic that isn't about the
//! terminal lives in the library modules, with tests.

pub mod args;
mod commands;
mod context;
mod matching;
mod output;
mod tui;

use std::process::ExitCode;

use clap::Parser;

use self::args::{CacheCommand, Cli, Command, TerminalCommand};
use self::context::Context;
use self::output::Output;
use crate::cancel::{CancelToken, is_cancelled};

/// A mistake in how the command was used (exit code 2, like clap's own).
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct UsageError(pub String);

/// How a command ended, beyond plain errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    /// Some aspects of an apply worked and some didn't.
    Partial,
    /// The command reported its own failure.
    Failure,
}

pub const EXIT_FAILURE: u8 = 1;
pub const EXIT_USAGE: u8 = 2;
pub const EXIT_PARTIAL: u8 = 3;

pub fn run() -> ExitCode {
    let cli = Cli::parse();
    let out = Output::detect();
    let cancel = CancelToken::new();
    install_ctrl_c(cancel.clone(), out);

    let ctx = Context::new(cli.dry_run, cli.data_dir, out, cancel);
    match dispatch(&ctx, cli.command) {
        Ok(Outcome::Success) => ExitCode::SUCCESS,
        Ok(Outcome::Partial) => ExitCode::from(EXIT_PARTIAL),
        Ok(Outcome::Failure) => ExitCode::from(EXIT_FAILURE),
        Err(error) if is_cancelled(&error) => {
            out.error("Cancelled. Nothing was left half-done.");
            ExitCode::from(EXIT_FAILURE)
        }
        Err(error) if is_usage_error(&error) => {
            out.error(&error.to_string());
            ExitCode::from(EXIT_USAGE)
        }
        Err(error) => {
            out.error(&format!("{error:#}"));
            ExitCode::from(EXIT_FAILURE)
        }
    }
}

/// Wrong use of the command line, including a theme argument that fits several themes.
fn is_usage_error(error: &anyhow::Error) -> bool {
    error.is::<UsageError>() || matches!(error.downcast_ref(), Some(matching::MatchError::Ambiguous { .. }))
}

/// The first Ctrl-C asks running work to stop at the next safe point (between files or chunks), so
/// nothing is left half-installed; a second one exits at once.
fn install_ctrl_c(cancel: CancelToken, out: Output) {
    let _ = ctrlc::set_handler(move || {
        if cancel.is_cancelled() {
            std::process::exit(130);
        }
        cancel.cancel();
        if out.stderr_tty {
            eprintln!("\nCancelling… (press Ctrl-C again to quit at once)");
        }
    });
}

fn dispatch(ctx: &Context, command: Command) -> anyhow::Result<Outcome> {
    match command {
        Command::List(args) => commands::catalog::list(ctx, &args),
        Command::Show(args) => commands::catalog::show(ctx, &args),
        Command::Download(args) => commands::library::download(ctx, &args),
        Command::Remove(args) => commands::library::remove(ctx, &args),
        Command::Apply(args) => commands::desktop::apply(ctx, &args),
        Command::Current(args) => commands::desktop::current(ctx, args.json),
        Command::Wallpaper(args) => commands::desktop::wallpaper(ctx, &args),
        Command::Restore(args) => commands::desktop::restore(ctx, &args),
        Command::Terminal(TerminalCommand::Apps(args)) => commands::terminal::apps(ctx, args.json),
        Command::Terminal(TerminalCommand::Add(args)) => commands::terminal::add(ctx, &args),
        Command::Terminal(TerminalCommand::Remove(args)) => commands::terminal::remove(ctx, &args),
        Command::Cache(CacheCommand::Clear) => commands::library::clear_cache(ctx),
        Command::Paths(args) => commands::library::paths(ctx, args.json),
        Command::Tui => tui::run(ctx),
    }
}
