use super::event::ContractEvent;
use alloy_json_abi::JsonAbi;
use serde::{Deserialize, Serialize};

/// Configuration for a specific contract to index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractConfig {
    pub name: String,
    pub address: String,
    pub events: Vec<ContractEvent>,
    pub abi: Option<JsonAbi>,
}

impl ContractConfig {
    /// Create a new ContractConfig with event signatures
    pub fn new(
        name: String,
        address: String,
        event_signatures: Vec<&str>,
        abi: Option<JsonAbi>,
    ) -> Result<Self, String> {
        let events = event_signatures
            .into_iter()
            .map(|sig| ContractEvent::from_signature(sig, abi.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            name,
            address,
            events,
            abi,
        })
    }
}

/// Main configuration for the indexer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerConfig {
    pub name: String,
    pub description: String,
    pub network_id: u64,
    pub start_block: u64,
    pub contracts: Vec<ContractConfig>,
    pub custom_schema: Option<String>,
    pub custom_handlers: Option<String>,
}

impl IndexerConfig {
    /// Create a new IndexerConfig with builder pattern
    pub fn builder() -> IndexerConfigBuilder {
        IndexerConfigBuilder::default()
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate network
        crate::network::validate_network(self.network_id)?;

        // Validate contracts
        if self.contracts.is_empty() {
            return Err("At least one contract must be specified".to_string());
        }

        // Validate start block
        if self.start_block == 0 {
            return Err("Start block must be greater than 0".to_string());
        }

        Ok(())
    }
}

/// Builder for IndexerConfig
#[derive(Default)]
pub struct IndexerConfigBuilder {
    name: Option<String>,
    description: Option<String>,
    network_id: Option<u64>,
    start_block: Option<u64>,
    contracts: Vec<ContractConfig>,
    custom_schema: Option<String>,
    custom_handlers: Option<String>,
}

impl IndexerConfigBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn network(mut self, network_id: u64) -> Self {
        self.network_id = Some(network_id);
        self
    }

    pub fn start_block(mut self, block: u64) -> Self {
        self.start_block = Some(block);
        self
    }

    pub fn add_contract(mut self, contract: ContractConfig) -> Self {
        self.contracts.push(contract);
        self
    }

    pub fn custom_schema(mut self, schema: impl Into<String>) -> Self {
        self.custom_schema = Some(schema.into());
        self
    }

    pub fn custom_handlers(mut self, handlers: impl Into<String>) -> Self {
        self.custom_handlers = Some(handlers.into());
        self
    }

    pub fn build(self) -> Result<IndexerConfig, String> {
        let config = IndexerConfig {
            name: self.name.ok_or("Name is required")?,
            description: self.description.ok_or("Description is required")?,
            network_id: self.network_id.ok_or("Network ID is required")?,
            start_block: self.start_block.ok_or("Start block is required")?,
            contracts: self.contracts,
            custom_schema: self.custom_schema,
            custom_handlers: self.custom_handlers,
        };

        config.validate()?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contract_config() -> Result<(), String> {
        let contract = ContractConfig::new(
            "ERC20".to_string(),
            "0x1234...".to_string(),
            vec![
                "Transfer(address indexed from, address indexed to, uint256 value)",
                "Approval(address indexed owner, address indexed spender, uint256 value)",
            ],
            None,
        )?;

        assert_eq!(contract.name, "ERC20");
        assert_eq!(contract.events.len(), 2);
        assert_eq!(contract.events[0].name, "Transfer");
        assert_eq!(contract.events[1].name, "Approval");
        Ok(())
    }

    #[test]
    fn test_indexer_config_builder() -> Result<(), String> {
        let contract = ContractConfig::new(
            "ERC20".to_string(),
            "0x1234...".to_string(),
            vec!["Transfer(address indexed from, address indexed to, uint256 value)"],
            None,
        )?;

        let config = IndexerConfig::builder()
            .name("MyIndexer")
            .description("Test indexer")
            .network(1)
            .start_block(1_000_000)
            .add_contract(contract)
            .build()?;

        assert_eq!(config.name, "MyIndexer");
        assert_eq!(config.network_id, 1);
        assert_eq!(config.contracts.len(), 1);
        assert_eq!(config.start_block, 1_000_000);
        Ok(())
    }

    #[test]
    fn test_indexer_config_validation() {
        // Test empty contracts
        let config = IndexerConfig::builder()
            .name("Test")
            .description("Test")
            .network(1)
            .start_block(1)
            .build();
        assert!(config.is_err());

        // Test invalid start block
        let contract = ContractConfig::new(
            "Test".to_string(),
            "0x1234...".to_string(),
            vec!["Transfer(address indexed from, address indexed to, uint256 value)"],
            None,
        )
        .unwrap();

        let config = IndexerConfig::builder()
            .name("Test")
            .description("Test")
            .network(1)
            .start_block(0)
            .add_contract(contract)
            .build();
        assert!(config.is_err());
    }
}
