use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

const SKIPPED_DIRS: &[&str] = &[".git", "target"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceFile {
    pub absolute_path: PathBuf,
    pub relative_path: String,
    pub file_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFileReference {
    pub absolute_path: PathBuf,
    pub relative_path: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedText {
    pub text: String,
    pub references: Vec<ResolvedFileReference>,
}

#[derive(Debug, Clone, Default)]
pub struct FileIndex {
    files: Vec<WorkspaceFile>,
}

impl FileIndex {
    pub fn discover(root: &Path) -> Result<Self> {
        let mut files = Vec::new();
        collect_files(root, root, &mut files)?;
        files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        Ok(Self { files })
    }

    pub fn resolve(&self, reference: &str) -> Result<ResolvedFileReference> {
        let normalized = reference.trim();
        if normalized.is_empty() {
            bail!("file reference is empty");
        }

        let exact = self
            .files
            .iter()
            .find(|file| file.relative_path == normalized)
            .cloned();

        let candidate = if let Some(exact) = exact {
            exact
        } else {
            let prefix_matches: Vec<_> = self
                .files
                .iter()
                .filter(|file| {
                    file.relative_path.starts_with(normalized) || file.file_name.starts_with(normalized)
                })
                .cloned()
                .collect();

            match prefix_matches.as_slice() {
                [file] => file.clone(),
                [] => bail!("no file matches @{normalized}"),
                _ => {
                    let options = prefix_matches
                        .iter()
                        .take(6)
                        .map(|file| file.relative_path.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    bail!("@{normalized} is ambiguous: {options}");
                }
            }
        };

        let content = fs::read_to_string(&candidate.absolute_path).with_context(|| {
            format!("failed to read referenced file {}", candidate.absolute_path.display())
        })?;

        Ok(ResolvedFileReference {
            absolute_path: candidate.absolute_path,
            relative_path: candidate.relative_path,
            content,
        })
    }

    pub fn suggest(&self, prefix: &str) -> Vec<WorkspaceFile> {
        let prefix = prefix.trim();
        let mut matches: Vec<_> = self
            .files
            .iter()
            .filter(|file| {
                prefix.is_empty()
                    || file.relative_path.starts_with(prefix)
                    || file.file_name.starts_with(prefix)
            })
            .cloned()
            .collect();
        matches.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        matches.truncate(12);
        matches
    }

    pub fn expand_references(&self, input: &str) -> Result<ExpandedText> {
        let mut references = Vec::new();
        let mut rewritten_tokens = Vec::new();

        for token in input.split_whitespace() {
            if let Some(reference) = token.strip_prefix('@') {
                let resolved = self.resolve(reference)?;
                rewritten_tokens.push(resolved.relative_path.clone());
                references.push(resolved);
            } else {
                rewritten_tokens.push(token.to_string());
            }
        }

        if references.is_empty() {
            return Ok(ExpandedText {
                text: input.to_string(),
                references,
            });
        }

        let mut text = rewritten_tokens.join(" ");
        text.push_str("\n\nReferenced files:\n");
        for reference in &references {
            text.push_str(&format!(
                "[file:{}]\n{}\n[/file:{}]\n\n",
                reference.relative_path, reference.content, reference.relative_path
            ));
        }

        Ok(ExpandedText { text, references })
    }
}

fn collect_files(root: &Path, current: &Path, files: &mut Vec<WorkspaceFile>) -> Result<()> {
    for entry in fs::read_dir(current)
        .with_context(|| format!("failed to read directory {}", current.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();

        if path.is_dir() {
            if SKIPPED_DIRS.contains(&file_name.as_ref()) {
                continue;
            }

            collect_files(root, &path, files)?;
            continue;
        }

        if !path.is_file() {
            continue;
        }

        let relative_path = path
            .strip_prefix(root)
            .map_err(|error| anyhow!(error))?
            .to_string_lossy()
            .replace('\\', "/");

        files.push(WorkspaceFile {
            absolute_path: path,
            relative_path,
            file_name: file_name.to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::FileIndex;

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nanos))
    }

    use std::path::PathBuf;

    #[test]
    fn expands_chat_file_references_into_prompt_text() {
        let root = unique_temp_dir("mortimmy_tui_file_index");
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join("README.md"), "hello world\n").unwrap();
        std::fs::write(root.join("docs").join("guide.md"), "guide body\n").unwrap();

        let index = FileIndex::discover(&root).unwrap();
        let expanded = index
            .expand_references("summarize @README.md and @docs/guide.md")
            .unwrap();

        assert!(expanded.text.contains("summarize README.md and docs/guide.md"));
        assert!(expanded.text.contains("[file:README.md]"));
        assert!(expanded.text.contains("hello world"));
        assert!(expanded.text.contains("[file:docs/guide.md]"));
        assert_eq!(expanded.references.len(), 2);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn suggests_files_by_prefix_or_filename() {
        let root = unique_temp_dir("mortimmy_tui_file_suggest");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("README.md"), "hello world\n").unwrap();
        std::fs::write(root.join("src").join("main.rs"), "fn main() {}\n").unwrap();

        let index = FileIndex::discover(&root).unwrap();
        let readme = index.suggest("READ");
        assert_eq!(readme[0].relative_path, "README.md");

        let main_rs = index.suggest("main");
        assert_eq!(main_rs[0].relative_path, "src/main.rs");

        let _ = std::fs::remove_dir_all(root);
    }
}
