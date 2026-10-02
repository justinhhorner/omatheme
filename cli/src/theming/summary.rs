use serde::Serialize;

use super::models::{ApplyResult, ApplyStep, StepOutcome};
use crate::catalog::default_themes::uppercase_first;
use crate::palette::AppearanceMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SummaryKind {
    Success,
    Info,
    Warning,
    Error,
}

/// The human-readable outcome of an apply, worded as in both apps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplySummary {
    pub kind: SummaryKind,
    pub title: String,
    pub message: String,
}

impl ApplySummary {
    pub fn new(kind: SummaryKind, title: impl Into<String>, message: impl Into<String>) -> Self {
        ApplySummary { kind, title: title.into(), message: message.into() }
    }

    /// `accent_note` is the backend's note, added when the accent color changed.
    pub fn describe(result: &ApplyResult, theme_name: &str, mode: AppearanceMode, accent_note: Option<&str>) -> Self {
        if let Some(save) = result.result_for(ApplyStep::SaveOriginal)
            && save.outcome == StepOutcome::Failed
        {
            return Self::new(
                SummaryKind::Error,
                "Theme not applied",
                save.error.clone().unwrap_or_else(|| "Couldn't save your current desktop.".into()),
            );
        }

        let applied: Vec<&str> = result
            .steps
            .iter()
            .filter(|s| s.outcome == StepOutcome::Applied && s.step != ApplyStep::SaveOriginal)
            .map(|s| label(s.step, mode))
            .collect();
        let failed: Vec<_> = result.steps.iter().filter(|s| s.outcome == StepOutcome::Failed).collect();
        let accent_applied =
            result.result_for(ApplyStep::AccentColor).is_some_and(|s| s.outcome == StepOutcome::Applied);

        if failed.is_empty() {
            if applied.is_empty() {
                return Self::new(
                    SummaryKind::Info,
                    "Nothing changed",
                    format!("All options were turned off, so {theme_name} wasn't applied."),
                );
            }
            let mut message = format!("Updated {}.", join_list(&applied));
            if accent_applied && let Some(note) = accent_note {
                message += &format!(" {note}");
            }
            return Self::new(SummaryKind::Success, format!("{theme_name} applied"), message);
        }

        let problems = failed
            .iter()
            .map(|f| {
                format!("{}: {}", uppercase_first(label(f.step, mode)), f.error.as_deref().unwrap_or("Unknown error."))
            })
            .collect::<Vec<_>>()
            .join(" ");
        if applied.is_empty() {
            Self::new(SummaryKind::Error, format!("Couldn't apply {theme_name}"), problems)
        } else {
            Self::new(
                SummaryKind::Warning,
                format!("{theme_name} partly applied"),
                format!("Updated {}. {problems}", join_list(&applied)),
            )
        }
    }
}

pub fn label(step: ApplyStep, mode: AppearanceMode) -> &'static str {
    match step {
        ApplyStep::Wallpaper => "the wallpaper",
        ApplyStep::AppearanceMode if mode == AppearanceMode::Light => "light mode",
        ApplyStep::AppearanceMode => "dark mode",
        ApplyStep::AccentColor => "the accent color",
        ApplyStep::SaveOriginal => "your saved desktop",
    }
}

pub fn join_list(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => one.to_string(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theming::StepResult;

    fn result(steps: &[(ApplyStep, StepOutcome, Option<&str>)]) -> ApplyResult {
        ApplyResult {
            steps: steps
                .iter()
                .map(|(s, o, e)| StepResult { step: *s, outcome: *o, error: e.map(str::to_string) })
                .collect(),
        }
    }

    use ApplyStep::*;
    use StepOutcome::*;

    #[test]
    fn success_lists_what_changed() {
        let summary = ApplySummary::describe(
            &result(&[
                (SaveOriginal, Applied, None),
                (Wallpaper, Applied, None),
                (AppearanceMode, Applied, None),
                (AccentColor, Applied, None),
            ]),
            "Tokyo Night",
            crate::palette::AppearanceMode::Dark,
            None,
        );

        assert_eq!(summary.kind, SummaryKind::Success);
        assert_eq!(summary.title, "Tokyo Night applied");
        assert_eq!(summary.message, "Updated the wallpaper, dark mode, and the accent color.");
    }

    #[test]
    fn success_adds_the_backends_accent_note() {
        let summary = ApplySummary::describe(
            &result(&[(AccentColor, Applied, None)]),
            "Snow",
            crate::palette::AppearanceMode::Light,
            Some("Sign out to see it everywhere."),
        );
        assert_eq!(summary.message, "Updated the accent color. Sign out to see it everywhere.");

        let without_accent = ApplySummary::describe(
            &result(&[(Wallpaper, Applied, None), (AccentColor, SkippedByUser, None)]),
            "Snow",
            crate::palette::AppearanceMode::Light,
            Some("Sign out to see it everywhere."),
        );
        assert_eq!(without_accent.message, "Updated the wallpaper.");
    }

    #[test]
    fn wallpaper_only_success() {
        let summary = ApplySummary::describe(
            &result(&[
                (Wallpaper, Applied, None),
                (AppearanceMode, NotSupported, None),
                (AccentColor, NotSupported, None),
            ]),
            "Snow",
            crate::palette::AppearanceMode::Light,
            None,
        );
        assert_eq!(summary.kind, SummaryKind::Success);
        assert_eq!(summary.message, "Updated the wallpaper.");
    }

    #[test]
    fn partial_failure_is_a_warning_with_the_reason() {
        let summary = ApplySummary::describe(
            &result(&[
                (Wallpaper, Applied, None),
                (AppearanceMode, Applied, None),
                (AccentColor, Failed, Some("Access denied.")),
            ]),
            "Snow",
            crate::palette::AppearanceMode::Light,
            None,
        );
        assert_eq!(summary.kind, SummaryKind::Warning);
        assert_eq!(summary.message, "Updated the wallpaper and light mode. The accent color: Access denied.");
    }

    #[test]
    fn total_failure_is_an_error() {
        let summary = ApplySummary::describe(
            &result(&[(Wallpaper, Failed, Some("File missing.")), (AppearanceMode, SkippedByUser, None)]),
            "Snow",
            crate::palette::AppearanceMode::Light,
            None,
        );
        assert_eq!(summary.kind, SummaryKind::Error);
        assert_eq!(summary.title, "Couldn't apply Snow");
    }

    #[test]
    fn snapshot_failure_explains_nothing_changed() {
        let summary = ApplySummary::describe(
            &result(&[
                (SaveOriginal, Failed, Some("Couldn't save your current desktop, so nothing was changed.")),
                (Wallpaper, NotAttempted, None),
            ]),
            "Snow",
            crate::palette::AppearanceMode::Light,
            None,
        );
        assert_eq!(summary.kind, SummaryKind::Error);
        assert!(summary.message.contains("nothing was changed"));
    }

    #[test]
    fn everything_skipped_is_informational() {
        let summary = ApplySummary::describe(
            &result(&[(Wallpaper, SkippedByUser, None), (AppearanceMode, SkippedByUser, None)]),
            "Snow",
            crate::palette::AppearanceMode::Light,
            None,
        );
        assert_eq!(summary.kind, SummaryKind::Info);
    }
}
