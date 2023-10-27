use anyhow::Result;
use clap::Parser;
use configure::{parse_config, CONFIG_FILE_PATH};

#[derive(Debug, Parser)]
pub struct Command {
    // config file path
    #[clap(
        name = "path",
        long,
        short,
        value_name = "file path",
        default_value = "setting/config.toml"
    )]
    config_path: Option<String>,
}

impl Command {
    pub fn run(&self) -> Result<()> {
        if let Some(file_path) = &self.config_path {
            println!("config file path: {}", file_path);
            let _ = parse_config(file_path);
            CONFIG_FILE_PATH.write().clone_from(file_path);
        }
        Ok(())
    }
}
