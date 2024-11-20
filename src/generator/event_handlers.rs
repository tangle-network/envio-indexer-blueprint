use crate::indexer_utils::{ContractConfig, ContractEvent, IndexerConfig};
use std::path::PathBuf;

pub struct EventHandlerGenerator<'a> {
    config: &'a IndexerConfig,
    output_dir: &'a PathBuf,
}

impl<'a> EventHandlerGenerator<'a> {
    pub fn new(config: &'a IndexerConfig, output_dir: &'a PathBuf) -> Self {
        Self { config, output_dir }
    }

    pub fn generate(&self) -> Result<(), String> {
        let handlers_dir = self.output_dir.join("handlers");
        std::fs::create_dir_all(&handlers_dir).map_err(|e| e.to_string())?;

        // Generate types file
        self.generate_types(&handlers_dir)?;

        // Generate handlers for each contract
        for contract in &self.config.contracts {
            let contract_dir = handlers_dir.join(&contract.name);
            std::fs::create_dir_all(&contract_dir).map_err(|e| e.to_string())?;

            // Generate event handlers for this contract
            for event in &contract.events {
                self.generate_event_handler(&contract_dir, contract, event)?;
            }

            // Generate index file for contract
            self.generate_contract_index(&contract_dir, contract)?;
        }

        // Generate main index file
        self.generate_main_index(&handlers_dir)?;

        Ok(())
    }

    fn generate_types(&self, handlers_dir: &PathBuf) -> Result<(), String> {
        let types_content = r#"
export interface Event {
    id: string;
    blockNumber: number;
    blockHash: string;
    transactionHash: string;
    contractAddress: string;
    params: Record<string, any>;
}

export interface IndexerContext {
    indexerId: string;
    config: any;
    store: any;
}
"#;
        std::fs::write(handlers_dir.join("types.ts"), types_content).map_err(|e| e.to_string())
    }

    fn generate_event_handler(
        &self,
        contract_dir: &PathBuf,
        contract: &ContractConfig,
        event: &ContractEvent,
    ) -> Result<(), String> {
        let handler_content = format!(
            r#"import {{ Event, IndexerContext }} from '../types';

export async function handle{event_name}(
    event: Event,
    context: IndexerContext
): Promise<void> {{
    const {{ {params} }} = event.params;
    
    // TODO: Implement event handling logic
    // Available context:
    // - context.indexerId: unique identifier for this indexer instance
    // - context.config: indexer configuration
    // - context.store: database access
}}
"#,
            event_name = event.name,
            params = event
                .inputs
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        );

        std::fs::write(
            contract_dir.join(format!("{}.ts", event.name.to_lowercase())),
            handler_content,
        )
        .map_err(|e| e.to_string())
    }

    fn generate_contract_index(
        &self,
        contract_dir: &PathBuf,
        contract: &ContractConfig,
    ) -> Result<(), String> {
        let mut content = String::from("// Auto-generated event handler exports\n\n");

        for event in &contract.events {
            content.push_str(&format!(
                "export * from './{}';\n",
                event.name.to_lowercase()
            ));
        }

        std::fs::write(contract_dir.join("index.ts"), content).map_err(|e| e.to_string())
    }

    fn generate_main_index(&self, handlers_dir: &PathBuf) -> Result<(), String> {
        let mut content = String::from("// Auto-generated contract handler exports\n\n");

        for contract in &self.config.contracts {
            content.push_str(&format!(
                "export * as {} from './{}';\n",
                contract.name, contract.name
            ));
        }

        std::fs::write(handlers_dir.join("index.ts"), content).map_err(|e| e.to_string())
    }
}
