//! RP2350 BOOTSEL discovery and operator guidance.

use std::path::{Path, PathBuf};

use anyhow::Result;
use mortimmy_deploy::Bootsel;

use super::process;

/// Available BOOTSEL transport paths on the current machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootselTransport {
    Picotool,
    Volume(PathBuf),
}

/// Detect the best BOOTSEL transport available on the current host.
pub fn detect_transport(bootsel: &Bootsel) -> Result<Option<BootselTransport>> {
    if process::command_exists("picotool")
        && process::status_success(
            std::process::Command::new("picotool").arg("info"),
            "probe for BOOTSEL device with picotool",
        )?
    {
        return Ok(Some(BootselTransport::Picotool));
    }

    Ok(find_bootsel_volume(bootsel)?.map(BootselTransport::Volume))
}

/// Search `/Volumes` for a mounted BOOTSEL volume.
pub fn find_bootsel_volume(bootsel: &Bootsel) -> Result<Option<PathBuf>> {
    find_bootsel_volume_in(Path::new("/Volumes"), bootsel)
}

/// Render operator instructions for entering BOOTSEL mode.
pub fn instructions(bootsel: &Bootsel, rerun_command: &str) -> String {
    let mut rendered = String::from("Put the board into BOOTSEL mode before retrying:\n");

    for (index, step) in bootsel.manual_steps.iter().enumerate() {
        rendered.push_str(&format!("  {}. {}\n", index + 1, step));
    }

    rendered.push_str(&format!(
        "  {}. Re-run {}\n",
        bootsel.manual_steps.len() + 1,
        rerun_command
    ));

    rendered
}

fn find_bootsel_volume_in(root: &Path, bootsel: &Bootsel) -> Result<Option<PathBuf>> {
    if !root.is_dir() {
        return Ok(None);
    }

    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        if volume_matches(&path, bootsel)? {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn volume_matches(path: &Path, bootsel: &Bootsel) -> Result<bool> {
    if let Some(name) = path.file_name().and_then(|value| value.to_str())
        && bootsel.volume_labels.iter().any(|label| name.eq_ignore_ascii_case(label))
    {
        return Ok(true);
    }

    let info_path = path.join("INFO_UF2.TXT");
    if !info_path.is_file() {
        return Ok(false);
    }

    let contents = std::fs::read_to_string(&info_path)?;
    Ok(bootsel.info_tokens.iter().any(|token| contents.contains(token)))
}

#[cfg(test)]
mod tests {
    use super::{find_bootsel_volume_in, instructions};
    use mortimmy_deploy::Bootsel;

    fn sample_bootsel() -> Bootsel {
        Bootsel {
            button_name: "BOOTSEL",
            volume_labels: &["RP2350", "RPI-RP2"],
            info_tokens: &["RP2350", "Pico"],
            manual_steps: &[
                "Unplug the board.",
                "Hold BOOTSEL.",
                "Reconnect USB.",
            ],
        }
    }

    fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nanos))
    }

    #[test]
    fn detects_volume_by_label() {
        let root = unique_temp_dir("mortimmy_bootsel_label");
        let volume = root.join("RP2350");
        std::fs::create_dir_all(&volume).unwrap();

        let detected = find_bootsel_volume_in(&root, &sample_bootsel()).unwrap();
        assert_eq!(detected, Some(volume));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn detects_volume_by_info_file() {
        let root = unique_temp_dir("mortimmy_bootsel_info");
        let volume = root.join("MountedPico");
        std::fs::create_dir_all(&volume).unwrap();
        std::fs::write(volume.join("INFO_UF2.TXT"), "Model: RP2350 Bootloader").unwrap();

        let detected = find_bootsel_volume_in(&root, &sample_bootsel()).unwrap();
        assert_eq!(detected, Some(volume));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn prints_instructions_with_rerun_hint() {
        let rendered = instructions(&sample_bootsel(), "mortimmy-tools deploy firmware uf2-deploy");
        assert!(rendered.contains("1. Unplug the board."));
        assert!(rendered.contains("4. Re-run mortimmy-tools deploy firmware uf2-deploy"));
    }
}
