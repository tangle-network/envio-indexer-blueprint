use std::path::PathBuf;

use crate::indexer_utils::IndexerConfig;

pub struct SchemaGenerator<'a> {
    config: &'a IndexerConfig,
    output_dir: &'a PathBuf,
}

impl<'a> SchemaGenerator<'a> {
    pub fn new(config: &'a IndexerConfig, output_dir: &'a PathBuf) -> Self {
        Self { config, output_dir }
    }

    pub fn generate(&self) -> Result<(), String> {
        let schema_path = self.output_dir.join("schema.graphql");
        let mut schema = String::new();

        for contract in &self.config.contracts {
            for event in &contract.events {
                schema.push_str(&format!("type {} @entity {{\n", event.name));
                schema.push_str("  id: ID!\n");

                for param in &event.inputs {
                    let graphql_type = param.param_type.to_graphql_type()?;
                    schema.push_str(&format!("  {}: {}\n", param.name, graphql_type));
                }

                schema.push_str("}\n\n");
            }
        }

        std::fs::write(schema_path, schema).map_err(|e| e.to_string())?;
        Ok(())
    }
}
