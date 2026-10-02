//! `terminal apps|add|remove`.

use anyhow::Result;
use serde_json::json;

use super::super::args::TerminalArgs;
use super::super::context::Context;
use super::super::output::print_json;
use super::super::{Outcome, UsageError};
use crate::palette::TerminalColors;
use crate::terminals::{self, TerminalExporter};

/// The terminal `--app` names, else the one the app saved (`terminalApp`), else this OS's default.
fn pick<'a>(
    exporters: &'a [Box<dyn TerminalExporter>],
    asked: Option<&str>,
    saved: Option<&str>,
) -> Result<&'a dyn TerminalExporter, UsageError> {
    if let Some(asked) = asked {
        return exporters.iter().find(|e| e.id().eq_ignore_ascii_case(asked)).map(|e| e.as_ref()).ok_or_else(|| {
            UsageError(format!(
                "There's no terminal “{asked}” here. Use one of: {}.",
                exporters.iter().map(|e| e.id()).collect::<Vec<_>>().join(", ")
            ))
        });
    }
    Ok(exporters
        .iter()
        .find(|e| Some(e.id()) == saved)
        .or_else(|| exporters.iter().find(|e| e.id() == terminals::default_id()))
        .unwrap_or(&exporters[0])
        .as_ref())
}

pub fn apps(ctx: &Context, as_json: bool) -> Result<Outcome> {
    let exporters = ctx.terminal_exporters();
    let saved = ctx.settings().load().terminal_app;
    let selected = pick(&exporters, None, saved.as_deref())?.id();

    if as_json {
        let apps: Vec<_> = exporters
            .iter()
            .map(|e| {
                json!({
                    "id": e.id(),
                    "name": e.display_name(),
                    "installed": e.is_installed(),
                    "default": e.id() == selected,
                    "canRemove": e.can_remove(),
                })
            })
            .collect();
        print_json(&apps)?;
        return Ok(Outcome::Success);
    }

    let out = &ctx.out;
    for exporter in &exporters {
        let default = if exporter.id() == selected { out.green(" (default)") } else { String::new() };
        let installed = if exporter.is_installed() { String::new() } else { out.dim(" not installed") };
        println!("{:<17} {}{default}{installed}", exporter.id(), exporter.display_name());
        println!("{:<17} {}", "", out.dim(&exporter.add_hint()));
    }
    Ok(Outcome::Success)
}

/// The theme's name as the terminal shows it: the downloaded copy's, else the catalog's.
fn theme_name(
    ctx: &Context,
    args: &TerminalArgs,
) -> Result<(crate::catalog::CatalogEntry, String, Option<crate::palette::Palette>)> {
    let index = ctx.theme_index(false)?;
    ctx.show_notices(&index.notices);
    let entry = index.find(&args.theme)?.clone();
    let installed = index.installed(&entry.slug);
    let name = installed.map(|t| t.name.clone()).unwrap_or_else(|| entry.name.clone());
    let palette = installed.and_then(|t| t.palette.clone());
    Ok((entry, name, palette))
}

pub fn add(ctx: &Context, args: &TerminalArgs) -> Result<Outcome> {
    let exporters = ctx.terminal_exporters();
    let saved = ctx.settings().load().terminal_app;
    let exporter = pick(&exporters, args.app.as_deref(), saved.as_deref())?;
    if !exporter.is_installed() {
        anyhow::bail!("Install {} to use these colors in it.", exporter.display_name());
    }

    let (entry, name, saved_palette) = theme_name(ctx, args)?;
    let palette = ctx.palette_for_terminal(&entry, saved_palette.as_ref())?;
    exporter.add(&entry.slug, &name, &TerminalColors::from_palette(&palette))?;

    let scheme = exporter.scheme_name(&name);
    println!(
        "{} {}",
        ctx.out.green(&format!("Added to {}.", exporter.display_name())),
        exporter.added_message(&scheme)
    );
    if let Some(location) = exporter.location(&entry.slug, &name) {
        println!("{}", ctx.out.dim(&location.display().to_string()));
    }
    if ctx.dry_run {
        ctx.out.note("Dry run: the file was written inside the data folder and no app was opened.");
    }
    Ok(Outcome::Success)
}

pub fn remove(ctx: &Context, args: &TerminalArgs) -> Result<Outcome> {
    let exporters = ctx.terminal_exporters();
    let saved = ctx.settings().load().terminal_app;
    let exporter = pick(&exporters, args.app.as_deref(), saved.as_deref())?;
    let (entry, name, _) = theme_name(ctx, args)?;
    let scheme = exporter.scheme_name(&name);

    if !exporter.can_remove() {
        println!("{}", exporter.remove_instructions(&scheme));
        return Ok(Outcome::Failure);
    }
    if !exporter.is_added(&entry.slug, &name) {
        println!("{} doesn't have “{scheme}”.", exporter.display_name());
        return Ok(Outcome::Success);
    }
    exporter.remove(&entry.slug, &name)?;
    println!(
        "{} {}",
        ctx.out.bold(&format!("Removed from {}.", exporter.display_name())),
        exporter.removed_message(&scheme)
    );
    Ok(Outcome::Success)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminals::test_support::FakeMachine;

    #[test]
    fn the_app_flag_then_the_saved_choice_then_the_default() {
        let machine = FakeMachine::new(&[]);
        let exporters = terminals::all(machine.environment.clone());
        let first = exporters[0].id();
        let last = exporters.last().unwrap().id();

        assert_eq!(pick(&exporters, None, None).unwrap().id(), terminals::default_id());
        assert_eq!(pick(&exporters, None, Some(last)).unwrap().id(), last);
        assert_eq!(pick(&exporters, None, Some("unknown")).unwrap().id(), terminals::default_id());
        assert_eq!(pick(&exporters, Some(&first.to_uppercase()), Some(last)).unwrap().id(), first);
        assert!(pick(&exporters, Some("nope"), None).err().unwrap().0.contains("Use one of"));
    }
}
