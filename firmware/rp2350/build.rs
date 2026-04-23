use std::{env, fs, path::PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR not set"));
    let memory_x = include_bytes!("memory.x");
    let board_role_features = [
        (
            "board-motion-controller",
            "CARGO_FEATURE_BOARD_MOTION_CONTROLLER",
        ),
        (
            "board-audio-controller",
            "CARGO_FEATURE_BOARD_AUDIO_CONTROLLER",
        ),
    ];

    fs::write(out_dir.join("memory.x"), memory_x).expect("failed to write memory.x");

    let enabled_board_roles = board_role_features
        .iter()
        .filter_map(|(feature_name, env_name)| {
            env::var_os(env_name).is_some().then_some(*feature_name)
        })
        .collect::<Vec<_>>();

    if enabled_board_roles.len() != 1 {
        panic!(
            "expected exactly one board role feature, enabled: {:?}. Use one of: board-motion-controller, board-audio-controller",
            enabled_board_roles
        );
    }

    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rerun-if-changed=memory.x");
    for (_, env_name) in board_role_features {
        println!("cargo:rerun-if-env-changed={env_name}");
    }
}
