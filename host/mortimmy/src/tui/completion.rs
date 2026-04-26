use super::{
    commands::{self, COMMAND_SPECS},
    files::FileIndex,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub label: String,
    pub detail: String,
    pub replacement: String,
    pub append_space: bool,
    pub start: usize,
    pub end: usize,
}

pub fn suggestions(input: &str, cursor: usize, files: &FileIndex) -> Vec<Suggestion> {
    let cursor = cursor.min(input.len());
    let (start, end) = token_bounds(input, cursor);
    let token = &input[start..end];
    let tokens_before_current = input[..start].split_whitespace().count();
    let first_token = input.split_whitespace().next().unwrap_or_default();
    let command_name = first_token.trim_start_matches('/').to_ascii_lowercase();

    if start == 0 && token.starts_with('/') {
        let prefix = token.trim_start_matches('/').to_ascii_lowercase();
        return COMMAND_SPECS
            .iter()
            .filter(|spec| spec.name.starts_with(&prefix))
            .map(|spec| Suggestion {
                label: format!("/{}", spec.name),
                detail: spec.summary.to_string(),
                replacement: format!("/{}", spec.name),
                append_space: command_name_should_append_space(spec.name),
                start,
                end,
            })
            .collect();
    }

    if first_token.starts_with('/') && tokens_before_current == 1 {
        match command_name.as_str() {
            "help" => return command_name_suggestions(token, start, end),
            "mode" => return fixed_value_suggestions(commands::mode_names(), token, start, end, "mode value"),
            _ => {}
        }
    }

    if token.starts_with('@') {
        return files
            .suggest(token.trim_start_matches('@'))
            .into_iter()
            .map(|file| Suggestion {
                label: format!("@{}", file.relative_path),
                detail: "workspace file".to_string(),
                replacement: format!("@{}", file.relative_path),
                append_space: true,
                start,
                end,
            })
            .collect();
    }

    Vec::new()
}

fn command_name_suggestions(token: &str, start: usize, end: usize) -> Vec<Suggestion> {
    let prefix = token.trim_start_matches('/').to_ascii_lowercase();
    COMMAND_SPECS
        .iter()
        .filter(|spec| spec.name.starts_with(&prefix))
        .map(|spec| Suggestion {
            label: format!("/{}", spec.name),
            detail: spec.summary.to_string(),
            replacement: spec.name.to_string(),
            append_space: false,
            start,
            end,
        })
        .collect()
}

fn fixed_value_suggestions(
    values: &[&str],
    token: &str,
    start: usize,
    end: usize,
    detail: &str,
) -> Vec<Suggestion> {
    let prefix = token.to_ascii_lowercase();
    values
        .iter()
        .filter(|value| value.starts_with(&prefix))
        .map(|value| Suggestion {
            label: (*value).to_string(),
            detail: detail.to_string(),
            replacement: (*value).to_string(),
            append_space: fixed_value_should_append_space(value),
            start,
            end,
        })
        .collect()
}

pub fn apply_suggestion(input: &mut String, cursor: &mut usize, suggestion: &Suggestion) {
    input.replace_range(suggestion.start..suggestion.end, &suggestion.replacement);
    let mut next_cursor = suggestion.start + suggestion.replacement.len();

    if suggestion.append_space
        && (next_cursor == input.len() || !input.as_bytes()[next_cursor].is_ascii_whitespace())
    {
        input.insert(next_cursor, ' ');
        next_cursor += 1;
    }

    *cursor = next_cursor;
}

fn command_name_should_append_space(command_name: &str) -> bool {
    matches!(command_name, "help" | "mode" | "servo" | "drive" | "chat")
}

fn fixed_value_should_append_space(value: &str) -> bool {
    !matches!(value, "teleop" | "autonomous" | "fault")
}

fn token_bounds(input: &str, cursor: usize) -> (usize, usize) {
    let bytes = input.as_bytes();
    let mut start = cursor;
    while start > 0 && !bytes[start - 1].is_ascii_whitespace() {
        start -= 1;
    }

    let mut end = cursor;
    while end < bytes.len() && !bytes[end].is_ascii_whitespace() {
        end += 1;
    }

    (start, end)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{apply_suggestion, suggestions};
    use crate::tui::files::FileIndex;

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nanos))
    }

    #[test]
    fn suggests_slash_commands_from_first_token() {
        let suggestions = suggestions("/he", 3, &FileIndex::default());
        assert!(suggestions.iter().any(|suggestion| suggestion.label == "/help"));
    }

    #[test]
    fn suggests_help_topics_and_mode_values() {
        let file_index = FileIndex::default();

        let help_suggestions = suggestions("/help mo", 8, &file_index);
        assert!(help_suggestions.iter().any(|suggestion| suggestion.label == "/mode"));

        let mode_suggestions = suggestions("/mode au", 8, &file_index);
        assert!(mode_suggestions.iter().any(|suggestion| suggestion.label == "autonomous"));
    }

    #[test]
    fn applies_smart_trailing_space_for_contextual_completions() {
        let file_index = FileIndex::default();

        let command_suggestion = suggestions("/mo", 3, &file_index)
            .into_iter()
            .find(|suggestion| suggestion.label == "/mode")
            .unwrap();
        let mut input = "/mo".to_string();
        let mut cursor = input.len();
        apply_suggestion(&mut input, &mut cursor, &command_suggestion);
        assert_eq!(input, "/mode ");
        assert_eq!(cursor, input.len());

        let help_topic_suggestion = suggestions("/help mo", 8, &file_index)
            .into_iter()
            .find(|suggestion| suggestion.label == "/mode")
            .unwrap();
        let mut input = "/help mo".to_string();
        let mut cursor = input.len();
        apply_suggestion(&mut input, &mut cursor, &help_topic_suggestion);
        assert_eq!(input, "/help mode");
        assert_eq!(cursor, input.len());

        let mode_value_suggestion = suggestions("/mode au", 8, &file_index)
            .into_iter()
            .find(|suggestion| suggestion.label == "autonomous")
            .unwrap();
        let mut input = "/mode au".to_string();
        let mut cursor = input.len();
        apply_suggestion(&mut input, &mut cursor, &mode_value_suggestion);
        assert_eq!(input, "/mode autonomous");
        assert_eq!(cursor, input.len());

        let drive_command_suggestion = suggestions("/dr", 3, &file_index)
            .into_iter()
            .find(|suggestion| suggestion.label == "/drive")
            .unwrap();
        let mut input = "/dr".to_string();
        let mut cursor = input.len();
        apply_suggestion(&mut input, &mut cursor, &drive_command_suggestion);
        assert_eq!(input, "/drive ");
        assert_eq!(cursor, input.len());
    }

    #[test]
    fn applies_file_completion_to_current_token() {
        let root = unique_temp_dir("mortimmy_tui_complete");
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join("docs").join("guide.md"), "guide body\n").unwrap();

        let index = FileIndex::discover(&root).unwrap();
        let suggestion = suggestions("/chat explain @gui", 18, &index)
            .into_iter()
            .next()
            .unwrap();

        let mut input = "/chat explain @gui".to_string();
        let mut cursor = input.len();
        apply_suggestion(&mut input, &mut cursor, &suggestion);

        assert_eq!(input, "/chat explain @docs/guide.md ");
        assert_eq!(cursor, input.len());

        let _ = std::fs::remove_dir_all(root);
    }
}
