//! Terminal output: colors only on a TTY (and never with NO_COLOR or TERM=dumb), messages and
//! progress on stderr, confirmations that refuse to guess when there's no one to ask.

use std::io::{BufRead, IsTerminal, Write};

use serde::Serialize;

use super::UsageError;
use crate::color::RgbColor;

#[derive(Debug, Clone, Copy)]
pub struct Output {
    /// Styles on stdout.
    pub color: bool,
    /// Styles on stderr.
    pub err_color: bool,
    /// Progress bars go to stderr, only when it's a terminal.
    pub stderr_tty: bool,
    /// Confirmations need someone to answer.
    pub stdin_tty: bool,
}

impl Output {
    pub fn detect() -> Self {
        let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
            || std::env::var("TERM").is_ok_and(|t| t == "dumb");
        let stdout_tty = std::io::stdout().is_terminal();
        let stderr_tty = std::io::stderr().is_terminal();
        Output {
            color: stdout_tty && !no_color,
            err_color: stderr_tty && !no_color,
            stderr_tty,
            stdin_tty: std::io::stdin().is_terminal(),
        }
    }

    fn paint(enabled: bool, code: &str, text: &str) -> String {
        if enabled { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() }
    }

    pub fn bold(&self, text: &str) -> String {
        Self::paint(self.color, "1", text)
    }

    pub fn dim(&self, text: &str) -> String {
        Self::paint(self.color, "2", text)
    }

    pub fn green(&self, text: &str) -> String {
        Self::paint(self.color, "32", text)
    }

    pub fn yellow(&self, text: &str) -> String {
        Self::paint(self.color, "33", text)
    }

    pub fn red(&self, text: &str) -> String {
        Self::paint(self.color, "31", text)
    }

    pub fn cyan(&self, text: &str) -> String {
        Self::paint(self.color, "36", text)
    }

    /// A small block of `color` (truecolor) followed by a space, or nothing without colors.
    pub fn swatch(&self, color: RgbColor) -> String {
        if self.color { format!("\x1b[48;2;{};{};{}m  \x1b[0m ", color.r, color.g, color.b) } else { String::new() }
    }

    /// A note on stderr (stale data, what a dry run would have done).
    pub fn note(&self, message: &str) {
        eprintln!("{} {message}", Self::paint(self.err_color, "2", "note:"));
    }

    pub fn warn(&self, message: &str) {
        eprintln!("{} {message}", Self::paint(self.err_color, "33", "warning:"));
    }

    pub fn error(&self, message: &str) {
        eprintln!("{} {message}", Self::paint(self.err_color, "1;31", "error:"));
    }

    /// Asks on the terminal; refuses (as a usage error) when no one can answer, so a script never
    /// changes the desktop by accident.
    pub fn confirm(&self, question: &str, yes: bool) -> anyhow::Result<bool> {
        if yes {
            return Ok(true);
        }
        if !self.stdin_tty {
            return Err(UsageError(
                "This needs your confirmation and there's no terminal to ask. Pass --yes to go ahead.".into(),
            )
            .into());
        }
        eprint!("{question} [y/N] ");
        std::io::stderr().flush()?;
        let mut answer = String::new();
        std::io::stdin().lock().read_line(&mut answer)?;
        Ok(matches!(answer.trim().to_lowercase().as_str(), "y" | "yes"))
    }
}

pub fn print_json<T: Serialize + ?Sized>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/// "12.3 MB".
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1000 {
        return format!("{bytes} bytes");
    }
    let mut value = bytes as f64 / 1000.0;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// Pads `text` to `width` characters (by chars, not bytes).
pub fn pad(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if len >= width { text.to_string() } else { format!("{text}{}", " ".repeat(width - len)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain() -> Output {
        Output { color: false, err_color: false, stderr_tty: false, stdin_tty: false }
    }

    #[test]
    fn without_a_terminal_there_are_no_escape_codes() {
        let out = plain();
        assert_eq!(out.bold("x"), "x");
        assert_eq!(out.swatch(RgbColor::new(1, 2, 3)), "");
        let colored = Output { color: true, ..plain() };
        assert_eq!(colored.swatch(RgbColor::new(1, 2, 3)), "\x1b[48;2;1;2;3m  \x1b[0m ");
    }

    #[test]
    fn confirmation_without_a_terminal_needs_yes() {
        let error = plain().confirm("Apply?", false).err().unwrap();
        assert!(error.is::<UsageError>());
        assert!(plain().confirm("Apply?", true).unwrap());
    }

    #[test]
    fn bytes_are_human_readable() {
        assert_eq!(format_bytes(512), "512 bytes");
        assert_eq!(format_bytes(412_233), "412.2 KB");
        assert_eq!(format_bytes(12_345_678), "12.3 MB");
    }

    #[test]
    fn padding_counts_characters() {
        assert_eq!(pad("Rosé", 6), "Rosé  ");
        assert_eq!(pad("long name", 4), "long name");
    }
}
