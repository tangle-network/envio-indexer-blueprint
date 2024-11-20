use crate::indexer_utils::IndexerConfig;
use std::path::PathBuf;

mod config;
mod event_handlers;
mod schema;

pub struct IndexerGenerator<'a> {
    config: &'a IndexerConfig,
    output_dir: &'a PathBuf,
}

impl<'a> IndexerGenerator<'a> {
    pub fn new(config: &'a IndexerConfig, output_dir: &'a PathBuf) -> Self {
        Self { config, output_dir }
    }

    pub fn generate(&self) -> Result<(), String> {
        // Generate schema
        schema::SchemaGenerator::new(self.config, self.output_dir).generate()?;

        // Generate handlers
        event_handlers::EventHandlerGenerator::new(self.config, self.output_dir).generate()?;

        // Generate config
        config::ConfigGenerator::new(self.config, self.output_dir).generate()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexer_utils::{ContractConfig, IndexerConfig};

    #[test]
    fn test_generator() -> Result<(), String> {
        let temp_dir = tempfile::tempdir().map_err(|e| e.to_string())?;
        let output_dir = temp_dir.path().to_path_buf();

        let contract = ContractConfig::new(
            "ERC20".to_string(),
            "0x1234...".to_string(),
            vec!["Transfer(address indexed from, address indexed to, uint256 value)"],
            None,
        )?;

        let config = IndexerConfig::builder()
            .name("TestIndexer")
            .description("Test Description")
            .network(1)
            .start_block(1_000_000)
            .add_contract(contract)
            .build()?;

        let generator = IndexerGenerator::new(&config, &output_dir);
        generator.generate()?;

        // Verify files exist
        assert!(output_dir.join("schema.graphql").exists());
        assert!(output_dir.join("EventHandlers.ts").exists());
        assert!(output_dir.join("config.yaml").exists());

        Ok(())
    }
}
