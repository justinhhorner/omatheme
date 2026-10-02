//! Reads a TOML file into "section.key" → string pairs (non-string values are dropped; palettes only
//! need strings). Keys are lowercased so lookups are case-insensitive. Community theme files are
//! often hand-edited, so if strict parsing fails it falls back to a lenient line scanner rather
//! than rejecting the theme.

use std::collections::HashMap;

pub fn parse(text: &str) -> HashMap<String, String> {
    match text.parse::<toml::Table>() {
        Ok(table) => {
            let mut result = HashMap::new();
            flatten(&table, "", &mut result);
            result
        }
        Err(_) => parse_lenient(text),
    }
}

fn flatten(table: &toml::Table, prefix: &str, result: &mut HashMap<String, String>) {
    for (key, value) in table {
        let flat_key = format!("{prefix}{key}");
        match value {
            toml::Value::String(s) => {
                result.insert(flat_key.to_lowercase(), s.clone());
            }
            toml::Value::Table(inner) => flatten(inner, &format!("{flat_key}."), result),
            // Arrays, numbers, booleans and dates aren't palette values.
            _ => {}
        }
    }
}

/// `[section]` headers and `key = "value"` / `key = 'value'` lines; anything else is skipped.
pub fn parse_lenient(text: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let mut section = String::new();
    for raw_line in text.split(['\n', '\r']) {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(header) = section_header(line) {
            section = header.replace([' ', '"'], "") + ".";
            continue;
        }
        if let Some((key, value)) = key_value(line) {
            result.insert(format!("{section}{key}").to_lowercase(), value.to_string());
        }
    }
    result
}

/// `[ name ]` at the start of the line (not `[[name]]`).
fn section_header(line: &str) -> Option<&str> {
    let inner = line.strip_prefix('[')?;
    let end = inner.find(']')?;
    let name = inner[..end].trim();
    (!name.is_empty() && !name.contains('[')).then_some(name)
}

/// `key = "value"`, with a bare key ([A-Za-z0-9_-]) and either quote.
fn key_value(line: &str) -> Option<(&str, &str)> {
    let key_end = line.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-')).unwrap_or(line.len());
    if key_end == 0 {
        return None;
    }
    let rest = line[key_end..].trim_start().strip_prefix('=')?.trim_start();
    let rest = rest.strip_prefix(['"', '\''])?;
    let value_end = rest.find(['"', '\''])?;
    Some((&line[..key_end], &rest[..value_end]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_inline_tables_quoted_and_dotted_keys() {
        let values = parse(
            r##"
# comment
[colors]
primary = { background = "#1e1e2e", foreground = "#cdd6f4" } # trailing comment
"quoted key" = "a"
normal.blue = "#89b4fa"

[colors.bright]
red = 'literal \n kept'
"##,
        );

        assert_eq!(values["colors.primary.background"], "#1e1e2e");
        assert_eq!(values["colors.primary.foreground"], "#cdd6f4");
        assert_eq!(values["colors.quoted key"], "a");
        assert_eq!(values["colors.normal.blue"], "#89b4fa");
        assert_eq!(values["colors.bright.red"], "literal \\n kept");
    }

    #[test]
    fn handles_escapes_multiline_strings_and_skips_other_values() {
        let values = parse(
            r##"
name = "Rosé \"Pine\""
description = """
two
lines"""
size = 12.5
enabled = true
when = 1979-05-27 07:32:00Z
list = [
  "#000000", # comment
  ["nested"],
]
Accent = "#ABCDEF"
"##,
        );

        assert_eq!(values["name"], "Rosé \"Pine\"");
        assert_eq!(values["description"], "two\nlines");
        assert!(!values.contains_key("size"));
        assert!(!values.contains_key("list"));
        assert_eq!(values["accent"], "#ABCDEF"); // keys are case-insensitive
    }

    #[test]
    fn lenient_scanner_is_used_for_broken_files() {
        let values = parse("[colors.primary]\nbackground = \"#000000\"\noops =\nforeground = \"#ffffff\"\n");

        assert_eq!(values["colors.primary.background"], "#000000");
        assert_eq!(values["colors.primary.foreground"], "#ffffff");
    }

    /// Files edited on Windows end lines with CRLF.
    #[test]
    fn reads_windows_line_endings() {
        for text in [
            "[colors.primary]\r\nbackground = \"#000000\"\r\nforeground = \"#ffffff\"\r\n", // valid: strict reader
            "[colors.primary]\r\nbackground = \"#000000\"\r\noops =\r\nforeground = \"#ffffff\"\r\n", // broken: lenient
        ] {
            let values = parse(text);
            assert_eq!(values["colors.primary.background"], "#000000");
            assert_eq!(values["colors.primary.foreground"], "#ffffff");
        }
    }

    #[test]
    fn lenient_scanner_ignores_array_tables_and_unquoted_values() {
        let values = parse_lenient("[[x]]\na = \"1\"\nb = 2\n[ \"y z\" ]\nc = '3'");
        assert_eq!(values.get("a").map(String::as_str), Some("1"));
        assert!(!values.contains_key("b"));
        assert_eq!(values.get("yz.c").map(String::as_str), Some("3"));
    }
}
