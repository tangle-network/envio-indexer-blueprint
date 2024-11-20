use crate::indexer_utils::IndexerConfig;
use std::path::PathBuf;

pub struct ConfigGenerator<'a> {
    config: &'a IndexerConfig,
    output_dir: &'a PathBuf,
}

impl<'a> ConfigGenerator<'a> {
    pub fn new(config: &'a IndexerConfig, output_dir: &'a PathBuf) -> Self {
        Self { config, output_dir }
    }

    pub fn generate(&self) -> Result<(), String> {
        let config_path = self.output_dir.join("config.yaml");
        let mut yaml = String::new();

        yaml.push_str(&format!("name: {}\n", self.config.name));
        yaml.push_str(&format!("description: {}\n", self.config.description));
        yaml.push_str("networks:\n");
        yaml.push_str(&format!("  - id: {}\n", self.config.network_id));
        yaml.push_str(&format!("    start_block: {}\n", self.config.start_block));
        yaml.push_str("    contracts:\n");

        for contract in &self.config.contracts {
            yaml.push_str(&format!("      - name: {}\n", contract.name));
            yaml.push_str(&format!("        address: \"{}\"\n", contract.address));
            yaml.push_str("        handler: src/EventHandlers.ts\n");
            yaml.push_str("        events:\n");

            for event in &contract.events {
                yaml.push_str(&format!("          - event: \"{}\"\n", event.signature));
            }
        }

        yaml.push_str("rollback_on_reorg: false\n");

        std::fs::write(config_path, yaml).map_err(|e| e.to_string())?;
        Ok(())
    }
}
