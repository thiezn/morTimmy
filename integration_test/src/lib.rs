//! Hardware integration-test scaffolding for mortimmy.

pub mod config;

pub use config::{
	HardwareTestConfig, load_hardware_test_config, load_hardware_test_config_from_path,
};
