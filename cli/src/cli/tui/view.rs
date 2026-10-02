//! Drawing the TUI from the [`App`] state.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Widget, Wrap};

use super::app::{App, Dialog, Focus, Lookup, Preview};
use crate::catalog::CatalogEntry;
use crate::cli::output::format_bytes;
use crate::color::RgbColor;
use crate::palette::{AppearanceMode, TerminalColors};
use crate::terminals::scheme_name;
use crate::theming::SummaryKind;

const SPINNER: [&str; 4] = ["◐", "◓", "◑", "◒"];

fn dim() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}

fn bold() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

fn rgb(color: RgbColor) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}

fn kind_style(kind: SummaryKind) -> Style {
    match kind {
        SummaryKind::Success => Style::new().fg(Color::Green),
        SummaryKind::Info => Style::new(),
        SummaryKind::Warning => Style::new().fg(Color::Yellow),
        SummaryKind::Error => Style::new().fg(Color::Red),
    }
}

pub fn draw(frame: &mut Frame, app: &App, list_state: &mut ListState) {
    let [header, body, footer] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(3), Constraint::Length(2)]).areas(frame.area());
    let list_width = (body.width * 2 / 5).clamp(24.min(body.width), 44);
    let [list_area, details_area] =
        Layout::horizontal([Constraint::Length(list_width), Constraint::Min(10)]).areas(body);

    draw_header(frame, app, header);
    draw_list(frame, app, list_area, list_state);
    draw_details(frame, app, details_area);
    draw_footer(frame, app, footer);

    match &app.dialog {
        Some(Dialog::Apply(_)) => draw_apply(frame, app),
        Some(Dialog::Confirm(confirm)) => {
            let mut lines: Vec<Line> = confirm.lines.iter().map(|l| Line::raw(l.clone())).collect();
            if app.env.dry_run {
                lines.push(Line::styled("Dry run: nothing on the desktop changes.", Style::new().fg(Color::Yellow)));
            }
            lines.push(Line::raw(""));
            lines.push(Line::styled("⏎/y yes   Esc/n no", dim()));
            popup(frame, &confirm.title, lines, 64);
        }
        Some(Dialog::Terminal(_)) => draw_terminal(frame, app),
        Some(Dialog::Help) => draw_help(frame),
        None => {}
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let mut left = vec![Span::styled(" Omarchy Themes", bold())];
    if app.catalog_loading {
        left.push(Span::styled(format!("  {} loading themes…", SPINNER[app.tick % 4]), dim()));
    } else {
        left.push(Span::styled(format!("  {} of {} themes", app.visible.len(), app.entries.len()), dim()));
        if app.filter != super::app::Filter::All {
            left.push(Span::styled(format!(" · {}", app.filter.label()), dim()));
        }
    }
    if app.searching || !app.search.is_empty() {
        let cursor = if app.searching { "▏" } else { "" };
        left.push(Span::raw("  "));
        left.push(Span::styled(format!("/{}{cursor}", app.search), Style::new().fg(Color::Cyan)));
    }

    let mut right = Vec::new();
    if let Some(slug) = app.current_slug() {
        let name = app.installed_theme(slug).map(|t| t.name.as_str()).unwrap_or(slug);
        right.push(Span::styled("● ", Style::new().fg(Color::Green)));
        right.push(Span::raw(format!("{name} on the desktop ")));
    }
    let badge = match (app.env.dry_run, app.env.test_data_folder) {
        (true, true) => Some(" DRY RUN · TEST DATA FOLDER "),
        (true, false) => Some(" DRY RUN "),
        (false, true) => Some(" TEST DATA FOLDER "),
        _ => None,
    };
    if let Some(badge) = badge {
        right.push(Span::styled(badge, Style::new().fg(Color::Black).bg(Color::Yellow)));
    }
    let right_width: u16 = right.iter().map(|s| s.width() as u16).sum();
    let [l, r] = Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)]).areas(area);
    frame.render_widget(Paragraph::new(Line::from(left)), l);
    frame.render_widget(Paragraph::new(Line::from(right)), r);
}

fn draw_list(frame: &mut Frame, app: &App, area: Rect, state: &mut ListState) {
    let focused = app.focus == Focus::List && app.dialog.is_none();
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if focused { Style::new() } else { dim() })
        .title(" Themes ");

    let downloaded = app.downloaded();
    let defaults = app.visible.iter().filter(|&&i| app.entries[i].is_default_theme()).count();
    let community = app.visible.len() - defaults;
    let mut items = Vec::new();
    let mut selected_item = None;
    for (position, &index) in app.visible.iter().enumerate() {
        let entry = &app.entries[index];
        let first_default = position == 0 && entry.is_default_theme();
        let first_community =
            !entry.is_default_theme() && (position == 0 || app.entries[app.visible[position - 1]].is_default_theme());
        if first_default {
            items.push(ListItem::new(Line::styled(
                format!("Included with Omarchy ({defaults})"),
                bold().add_modifier(Modifier::DIM),
            )));
        }
        if first_community {
            if position > 0 {
                items.push(ListItem::new(""));
            }
            items.push(ListItem::new(Line::styled(
                format!("Community ({community})"),
                bold().add_modifier(Modifier::DIM),
            )));
        }
        if position == app.selected {
            selected_item = Some(items.len());
        }
        let marker = if app.current_slug() == Some(entry.slug.as_str()) {
            Span::styled("● ", Style::new().fg(Color::Green))
        } else if downloaded.contains(entry.slug.as_str()) {
            Span::styled("✓ ", Style::new().fg(Color::Cyan))
        } else {
            Span::raw("  ")
        };
        items.push(ListItem::new(Line::from(vec![marker, Span::raw(entry.name.clone())])));
    }
    if items.is_empty() {
        let text = if app.catalog_loading {
            "Loading…".to_string()
        } else if let Some(error) = &app.catalog_error {
            error.clone()
        } else {
            "No themes match.".to_string()
        };
        frame.render_widget(Paragraph::new(text).style(dim()).wrap(Wrap { trim: true }).block(block), area);
        return;
    }
    state.select(selected_item);
    let highlight =
        if focused { Style::new().add_modifier(Modifier::REVERSED) } else { Style::new().add_modifier(Modifier::BOLD) };
    let list = List::new(items).block(block).highlight_style(highlight);
    frame.render_stateful_widget(list, area, state);
}

fn draw_details(frame: &mut Frame, app: &App, area: Rect) {
    let Some(entry) = app.selected_entry() else {
        let block = Block::new().borders(Borders::ALL).border_type(BorderType::Rounded);
        frame.render_widget(block, area);
        return;
    };
    let installed = app.installed_theme(&entry.slug);
    let name = installed.map(|t| t.name.clone()).unwrap_or_else(|| entry.name.clone());
    let focused = app.focus == Focus::Wallpapers && app.dialog.is_none();
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if focused { Style::new() } else { dim() })
        .title(Line::styled(format!(" {name} "), bold()));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // The preview takes the top of the pane: 16:9 in half-block cells, at most half the height.
    let mut text_area = inner;
    if app.env.colors && inner.height >= 12 {
        let height = ((inner.width as u32 * 9 / 32) as u16).min(inner.height / 2).max(4);
        let [preview, rest] = Layout::vertical([Constraint::Length(height), Constraint::Min(0)]).areas(inner);
        match app.preview_key().and_then(|(key, _)| app.previews.get(&key)) {
            Some(Preview::Ready(image)) => frame.render_widget(HalfBlockImage(image), preview),
            Some(Preview::Loading) => frame.render_widget(
                Paragraph::new(format!("{} loading preview…", SPINNER[app.tick % 4])).style(dim()),
                preview,
            ),
            Some(Preview::Failed) => frame.render_widget(Paragraph::new("No preview.").style(dim()), preview),
            None => {}
        }
        text_area = Rect { y: rest.y + 1, height: rest.height.saturating_sub(1), ..rest };
    }

    let (lines, cursor_line) = detail_lines(app, entry, text_area.width);
    let scroll = cursor_line.map_or(0, |line| line.saturating_sub(text_area.height.saturating_sub(2) as usize)) as u16;
    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), text_area);
}

/// The details text, and the line the wallpaper cursor is on (for scrolling).
fn detail_lines<'a>(app: &'a App, entry: &'a CatalogEntry, width: u16) -> (Vec<Line<'a>>, Option<usize>) {
    let installed = app.installed_theme(&entry.slug);
    let mut lines = Vec::new();
    let source = if entry.is_default_theme() { "Included with Omarchy" } else { "Community" };
    lines.push(Line::styled(format!("{} · {source}", entry.slug), dim()));
    lines.push(Line::styled(entry.repo_url.clone(), dim()));

    let mut status = Vec::new();
    if let Some(mode) = app.mode(&entry.slug) {
        status.push(Span::raw(if mode == AppearanceMode::Light { "Light" } else { "Dark" }));
    }
    match installed {
        Some(theme) => {
            let when = theme.downloaded_at.with_timezone(&chrono::Local).format("%b %-d").to_string();
            status.push(Span::styled(format!("✓ Downloaded {when}"), Style::new().fg(Color::Cyan)));
        }
        None => status.push(Span::styled("Not downloaded", dim())),
    }
    if app.current_slug() == Some(entry.slug.as_str()) {
        status.push(Span::styled("● On the desktop", Style::new().fg(Color::Green)));
    }
    let mut joined = Vec::new();
    for (i, span) in status.into_iter().enumerate() {
        if i > 0 {
            joined.push(Span::styled(" · ", dim()));
        }
        joined.push(span);
    }
    lines.push(Line::from(joined));
    lines.push(Line::raw(""));

    let lookup = app.lookups.get(&entry.slug);
    if installed.is_none() {
        match lookup {
            Some(Lookup::Loading) => {
                lines.push(Line::styled(format!("{} Looking it up on GitHub…", SPINNER[app.tick % 4]), dim()));
                return (lines, None);
            }
            Some(Lookup::Failed(error)) => {
                lines.push(Line::styled(error.clone(), Style::new().fg(Color::Red)));
                lines.push(Line::styled("Press ⏎ to try again.", dim()));
                return (lines, None);
            }
            Some(Lookup::Loaded(_)) => {}
            None if entry.is_default_theme() => {
                lines.push(Line::styled("…", dim()));
                return (lines, None);
            }
            None => {
                lines.push(Line::styled("Press ⏎ to see its colors and wallpapers (one GitHub request).", dim()));
                return (lines, None);
            }
        }
    }

    match app.palette(&entry.slug) {
        Some(palette) => {
            lines.push(Line::styled(format!("Colors · {}", palette.source.file_name()), bold()));
            let mut key_colors = Vec::new();
            for (label, color) in
                [("Background", palette.background), ("Foreground", palette.foreground), ("Accent", palette.accent)]
            {
                key_colors.extend(swatch(app, color));
                key_colors.push(Span::raw(format!("{label} {color}   ")));
            }
            lines.push(Line::from(key_colors));
            if !palette.swatches.is_empty() {
                let per_line = (width as usize / 3).max(1);
                for chunk in palette.swatches.chunks(per_line) {
                    lines.push(Line::from(chunk.iter().flat_map(|s| swatch(app, s.color)).collect::<Vec<_>>()));
                }
            }
            let terminal = TerminalColors::from_palette(palette);
            lines.push(Line::raw(""));
            lines.push(Line::styled(format!("Terminal · “{}”", scheme_name(&entry.name)), bold()));
            for (label, colors) in [("normal ", &terminal.ansi[..8]), ("bright ", &terminal.ansi[8..])] {
                let mut row = vec![Span::styled(label, dim())];
                row.extend(colors.iter().flat_map(|c| swatch(app, *c)));
                lines.push(Line::from(row));
            }
        }
        None => {
            let error = app.details(&entry.slug).and_then(|d| d.palette_error.clone());
            lines.push(Line::styled(error.unwrap_or_else(|| "No palette.".into()), Style::new().fg(Color::Yellow)));
        }
    }

    lines.push(Line::raw(""));
    let mut cursor_line = None;
    match app.wallpapers(&entry.slug) {
        Some(wallpapers) if !wallpapers.is_empty() => {
            lines.push(Line::styled(format!("Wallpapers ({})", wallpapers.len()), bold()));
            let current = (app.current_slug() == Some(entry.slug.as_str()))
                .then_some(app.settings.last_applied_wallpaper.as_deref())
                .flatten();
            let focused = app.focus == Focus::Wallpapers && app.dialog.is_none();
            for (i, (name, size)) in wallpapers.into_iter().enumerate() {
                let at_cursor = focused && i == app.wallpaper_row;
                if at_cursor {
                    cursor_line = Some(lines.len());
                }
                let mut row = vec![
                    Span::raw(if at_cursor { "▸ " } else { "  " }),
                    Span::styled(format!("{:>2} ", i + 1), dim()),
                    Span::styled(
                        name.clone(),
                        if at_cursor { Style::new().add_modifier(Modifier::REVERSED) } else { Style::new() },
                    ),
                ];
                if let Some(size) = size {
                    row.push(Span::styled(format!("  {}", format_bytes(size)), dim()));
                }
                if current == Some(name.as_str()) {
                    row.push(Span::styled("  ● on the desktop", Style::new().fg(Color::Green)));
                }
                lines.push(Line::from(row));
            }
        }
        _ => lines.push(Line::styled("No wallpapers.", dim())),
    }
    (lines, cursor_line)
}

/// Two cells of `color`, or nothing without colors.
fn swatch(app: &App, color: RgbColor) -> Vec<Span<'static>> {
    if app.env.colors { vec![Span::styled("  ", Style::new().bg(rgb(color))), Span::raw(" ")] } else { Vec::new() }
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let [status, hints] = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);
    let line = if let Some(download) = &app.download {
        let file = if download.file_index < download.files {
            format!("{}/{}", download.file_index + 1, download.files)
        } else {
            "screenshot".to_string()
        };
        let mut spans = vec![Span::raw(format!(" {} Downloading {} {file} ", SPINNER[app.tick % 4], download.name))];
        match download.total {
            Some(total) if total > 0 => {
                let width = 24usize;
                let filled = ((download.bytes as f64 / total as f64) * width as f64).round() as usize;
                spans.push(Span::styled("━".repeat(filled.min(width)), Style::new().fg(Color::Cyan)));
                spans.push(Span::styled("━".repeat(width - filled.min(width)), dim()));
                spans.push(Span::raw(format!(" {} / {}", format_bytes(download.bytes), format_bytes(total))));
            }
            _ => spans.push(Span::raw(format_bytes(download.bytes))),
        }
        Line::from(spans)
    } else if let Some(message) = &app.message {
        Line::styled(format!(" {}", message.text), kind_style(message.kind))
    } else if let Some(notice) = app.notices.first() {
        Line::styled(format!(" {notice}"), Style::new().fg(Color::Yellow))
    } else {
        Line::raw("")
    };
    frame.render_widget(Paragraph::new(line), status);

    let keys: &[(&str, &str)] = if app.searching {
        &[("type", "search"), ("⏎", "keep"), ("Esc", "clear"), ("↑↓", "move")]
    } else if app.focus == Focus::Wallpapers {
        &[("↑↓", "wallpaper"), ("⏎", "use it"), ("a", "apply"), ("Esc", "back"), ("?", "help"), ("q", "quit")]
    } else {
        &[
            ("↑↓", "move"),
            ("⏎", "open"),
            ("/", "search"),
            ("o", "filter"),
            ("d", "download"),
            ("a", "apply"),
            ("t", "terminal"),
            ("?", "more"),
            ("q", "quit"),
        ]
    };
    let mut spans = vec![Span::raw(" ")];
    for (key, label) in keys {
        spans.push(Span::styled(*key, bold()));
        spans.push(Span::styled(format!(" {label}  "), dim()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), hints);
}

fn draw_apply(frame: &mut Frame, app: &App) {
    let Some(Dialog::Apply(dialog)) = &app.dialog else { return };
    let os = app.env.os_name;
    let mut lines = Vec::new();
    for row in 0..3 {
        let enabled = app.apply_row_enabled(dialog, row);
        let (label, on, detail) = match row {
            0 => {
                let wallpaper = dialog.wallpaper.clone().unwrap_or_else(|| "the first wallpaper".into());
                (
                    "Wallpaper",
                    dialog.options.wallpaper,
                    format!("{wallpaper} · {} ‹›", dialog.options.fit.display_name()),
                )
            }
            1 => {
                let mode = match dialog.mode {
                    Some(AppearanceMode::Light) => "light mode",
                    Some(AppearanceMode::Dark) => "dark mode",
                    None => "the theme's mode",
                };
                ("Light/dark", dialog.options.appearance_mode, format!("switch to {mode}"))
            }
            _ => {
                let accent = dialog.accent.map(|a| a.hex()).unwrap_or_else(|| "the theme's accent".into());
                ("Accent color", dialog.options.accent_color, accent)
            }
        };
        let supported = match (row, app.env.capabilities) {
            (_, None) => false,
            (0, Some(c)) => c.wallpaper,
            (1, Some(c)) => c.appearance_mode,
            (_, Some(c)) => c.accent_color,
        };
        let (check, detail) = if !supported {
            ("[–]", format!("not supported on {os}"))
        } else if !enabled {
            ("[–]", "the theme has none".to_string())
        } else {
            (if on { "[x]" } else { "[ ]" }, detail)
        };
        let pointer = if row == dialog.row { "▸ " } else { "  " };
        let style = if enabled { Style::new() } else { dim() };
        let style = if row == dialog.row { style.add_modifier(Modifier::BOLD) } else { style };
        lines.push(Line::from(vec![
            Span::raw(pointer),
            Span::styled(format!("{check} {label:<13}"), style),
            Span::styled(detail, if enabled { Style::new() } else { dim() }),
        ]));
    }
    lines.push(Line::raw(""));
    let unsupported = app.env.capabilities.is_some_and(|c| !c.appearance_mode || !c.accent_color);
    if let Some(hint) = app.env.appearance_hint.filter(|_| unsupported) {
        lines.push(Line::styled(format!("{os} doesn't let apps change these; use {hint}."), dim()));
    }
    if !dialog.downloaded {
        lines.push(Line::styled("It's downloaded first.", dim()));
    }
    if !app.has_snapshot {
        lines.push(Line::styled("Your current desktop is saved first, so r can put it back.", dim()));
    }
    if app.env.dry_run {
        lines.push(Line::styled("Dry run: nothing on the desktop changes.", Style::new().fg(Color::Yellow)));
    }
    lines.push(Line::styled("Space toggle   ←→ fit   ⏎ apply   Esc cancel", dim()));
    popup(frame, &format!("Apply {}", dialog.name), lines, 80);
}

fn draw_terminal(frame: &mut Frame, app: &App) {
    let Some(Dialog::Terminal(dialog)) = &app.dialog else { return };
    let mut lines = vec![
        Line::raw(format!("Adds “{}” to a terminal. Its settings aren't changed.", scheme_name(&dialog.name))),
        Line::raw(""),
    ];
    let default = app.default_terminal_row();
    for (i, terminal) in app.env.terminals.iter().enumerate() {
        let mut row = vec![
            Span::raw(if i == dialog.row { "▸ " } else { "  " }),
            Span::styled(
                format!("{:<18}", terminal.name),
                if i == dialog.row {
                    bold()
                } else if terminal.installed {
                    Style::new()
                } else {
                    dim()
                },
            ),
        ];
        if !terminal.installed {
            row.push(Span::styled("not installed  ", dim()));
        }
        if i == default {
            row.push(Span::styled("default", dim()));
        }
        lines.push(Line::from(row));
    }
    lines.push(Line::raw(""));
    if app.palette(&dialog.slug).is_none() {
        lines.push(Line::styled(format!("{} Loading the theme's colors…", SPINNER[app.tick % 4]), dim()));
    }
    lines.push(Line::styled("⏎ add   x remove   Esc close", dim()));
    popup(frame, &format!("Terminal colors for {}", dialog.name), lines, 66);
}

fn draw_help(frame: &mut Frame) {
    let keys = [
        ("↑ ↓  j k", "move (themes, or wallpapers)"),
        ("PgUp PgDn  g G", "page, first, last"),
        ("⏎ →", "open a theme (looks it up if it isn't downloaded)"),
        ("Esc ←", "back to the list"),
        ("/", "search by name, repo or slug"),
        ("o", "filter: all, downloaded, Omarchy's, community"),
        ("d", "download the theme"),
        ("a", "apply it (wallpaper, light/dark, accent)"),
        ("w  ⏎ on a wallpaper", "switch the wallpaper of the theme on the desktop"),
        ("t", "send its colors to a terminal"),
        ("x", "remove the download"),
        ("r", "restore your original desktop"),
        ("R", "check omarchy.org and GitHub for changes"),
        ("q  Ctrl-C", "quit"),
    ];
    let mut lines: Vec<Line> = keys
        .iter()
        .map(|(key, what)| Line::from(vec![Span::styled(format!("{key:<22}"), bold()), Span::raw(*what)]))
        .collect();
    lines.push(Line::raw(""));
    lines.push(Line::styled("✓ downloaded   ● on the desktop   any key closes", dim()));
    lines.push(Line::styled(
        "Themes come from Omarchy (omarchy.org) and its community. omatheme isn't affiliated with Omarchy or 37signals.",
        dim(),
    ));
    popup(frame, "Keys", lines, 76);
}

/// A centered box over everything else.
fn popup(frame: &mut Frame, title: &str, lines: Vec<Line>, width: u16) {
    let area = frame.area();
    let width = width.min(area.width.saturating_sub(2)).max(10);
    let inner_width = width.saturating_sub(4).max(1) as usize;
    let height: u16 = lines.iter().map(|l| (l.width().max(1).div_ceil(inner_width)) as u16).sum::<u16>() + 2;
    let height = height.min(area.height);
    let rect = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, rect);
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Line::styled(format!(" {title} "), bold()))
        .padding(ratatui::widgets::Padding::horizontal(1));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).block(block), rect);
}

/// An image drawn with "▀": each cell shows two pixels (foreground on top, background below),
/// scaled to fit and centered.
pub struct HalfBlockImage<'a>(pub &'a image::RgbImage);

impl Widget for HalfBlockImage<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (iw, ih) = self.0.dimensions();
        if area.width == 0 || area.height == 0 || iw == 0 || ih == 0 {
            return;
        }
        let (max_w, max_h) = (area.width as u32, area.height as u32 * 2);
        let scale = (max_w as f64 / iw as f64).min(max_h as f64 / ih as f64);
        let w = ((iw as f64 * scale).round() as u32).clamp(1, max_w);
        let h = ((ih as f64 * scale).round() as u32).clamp(2, max_h) & !1;
        let scaled = image::imageops::resize(self.0, w, h.max(2), image::imageops::FilterType::Triangle);
        let x0 = area.x + ((max_w - w) / 2) as u16;
        let y0 = area.y + ((max_h - h) / 4) as u16;
        for y in 0..h / 2 {
            for x in 0..w {
                let top = scaled.get_pixel(x, y * 2).0;
                let bottom = scaled.get_pixel(x, (y * 2 + 1).min(h - 1)).0;
                if let Some(cell) = buf.cell_mut((x0 + x as u16, y0 + y as u16)) {
                    cell.set_symbol("▀")
                        .set_fg(Color::Rgb(top[0], top[1], top[2]))
                        .set_bg(Color::Rgb(bottom[0], bottom[1], bottom[2]));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::super::app::tests::app;
    use super::super::app::{Lookup, Preview};
    use super::*;

    fn render(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let mut state = ListState::default();
        terminal.draw(|frame| draw(frame, app, &mut state)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area.height)
            .map(|y| (0..buffer.area.width).map(|x| buffer[(x, y)].symbol().to_string()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn press(app: &mut App, codes: &[KeyCode]) {
        for code in codes {
            app.handle_key(KeyEvent::new(*code, KeyModifiers::NONE), Instant::now());
        }
    }

    #[test]
    fn the_list_has_sections_and_markers_and_the_header_names_the_current_theme() {
        let screen = render(&app(), 110, 30);
        assert!(screen.contains("Included with Omarchy (2)"), "{screen}");
        assert!(screen.contains("Community (2)"));
        assert!(screen.contains("● Tokyo Night"));
        assert!(screen.contains("Tokyo Night on the desktop"));
        assert!(screen.contains("4 of 4 themes"));
    }

    #[test]
    fn a_downloaded_theme_shows_its_colors_and_wallpapers_from_disk() {
        let mut app = app();
        press(&mut app, &[KeyCode::Down]);
        let screen = render(&app, 110, 40);
        assert!(screen.contains("Background #1a1b26"), "{screen}");
        assert!(screen.contains("Terminal · “Tokyo Night (Omarchy)”"));
        assert!(screen.contains("Wallpapers (3)"));
        assert!(screen.contains("2.png  ● on the desktop"));
        assert!(screen.contains("● On the desktop"));
    }

    #[test]
    fn a_community_theme_waits_for_enter_and_then_shows_the_lookup() {
        let mut app = app();
        press(&mut app, &[KeyCode::End]);
        assert!(render(&app, 110, 30).contains("Press ⏎ to see its colors and wallpapers"));
        app.lookups.insert("vulkanite".into(), Lookup::Loading);
        assert!(render(&app, 110, 30).contains("Looking it up on GitHub"));
        app.lookups.insert("vulkanite".into(), Lookup::Failed("GitHub's rate limit was reached.".into()));
        assert!(render(&app, 110, 30).contains("GitHub's rate limit was reached."));
    }

    #[test]
    fn dialogs_say_what_will_happen() {
        let mut app = app();
        app.env.dry_run = true;
        press(&mut app, &[KeyCode::Down, KeyCode::Char('a')]);
        let screen = render(&app, 110, 30);
        assert!(screen.contains("Apply Tokyo Night"), "{screen}");
        assert!(screen.contains("[x] Wallpaper"));
        assert!(screen.contains("2.png · Fill Screen"));
        assert!(screen.contains("not supported on macOS"));
        assert!(screen.contains("Dry run: nothing on the desktop changes."));
        assert!(screen.contains("DRY RUN"));

        press(&mut app, &[KeyCode::Esc, KeyCode::Char('t')]);
        let screen = render(&app, 110, 30);
        assert!(screen.contains("Terminal colors for Tokyo Night"));
        assert!(screen.contains("not installed"));

        press(&mut app, &[KeyCode::Esc, KeyCode::Char('?')]);
        assert!(render(&app, 110, 40).contains("restore your original desktop"));
    }

    #[test]
    fn the_footer_shows_download_progress() {
        let mut app = app();
        app.download = Some(super::super::app::DownloadState {
            slug: "vulkanite".into(),
            name: "Vulkanite".into(),
            file_index: 1,
            files: 6,
            bytes: 1_500_000,
            total: Some(3_000_000),
        });
        let screen = render(&app, 110, 30);
        assert!(screen.contains("Downloading Vulkanite 2/6"), "{screen}");
        assert!(screen.contains("1.5 MB / 3.0 MB"));
    }

    #[test]
    fn previews_are_drawn_with_half_blocks() {
        let mut app = app();
        let image = image::RgbImage::from_pixel(32, 18, image::Rgb([0x7a, 0xa2, 0xf7]));
        app.previews.insert("screenshot:omarchy.catppuccin".into(), Preview::Ready(image));
        let mut terminal = Terminal::new(TestBackend::new(110, 40)).unwrap();
        let mut state = ListState::default();
        terminal.draw(|frame| draw(frame, &app, &mut state)).unwrap();
        let buffer = terminal.backend().buffer();
        let painted =
            buffer.content().iter().filter(|c| c.symbol() == "▀" && c.fg == Color::Rgb(0x7a, 0xa2, 0xf7)).count();
        assert!(painted > 100, "{painted} cells");
    }

    #[test]
    fn tiny_terminals_and_empty_lists_dont_panic() {
        let mut app = app();
        for (w, h) in [(10, 5), (30, 8), (1, 1)] {
            render(&app, w, h);
        }
        press(&mut app, &[KeyCode::Char('/'), KeyCode::Char('z'), KeyCode::Char('z')]);
        assert!(render(&app, 80, 20).contains("No themes match."));
    }
}
