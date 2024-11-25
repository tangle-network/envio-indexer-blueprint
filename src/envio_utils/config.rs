use crate::network::SUPPORTED_NETWORKS;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum ContractSource {
    Abi {
        abi: Option<String>,
        url: Option<String>,
    },
    Explorer {
        api_url: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContractDeployment {
    pub network_id: String,
    pub address: String,
    pub rpc_url: String,
    pub proxy_address: Option<String>,
    pub start_block: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContractConfig {
    pub name: String,
    pub source: ContractSource,
    pub deployments: Vec<ContractDeployment>,
}

impl ContractConfig {
    pub fn new(name: String, source: ContractSource, deployments: Vec<ContractDeployment>) -> Self {
        Self {
            name,
            source,
            deployments,
        }
    }

    pub fn add_deployment(
        &mut self,
        network_id: String,
        address: String,
        rpc_url: String,
        proxy_address: Option<String>,
        start_block: Option<u64>,
    ) {
        self.deployments.push(ContractDeployment {
            network_id,
            address,
            rpc_url,
            proxy_address,
            start_block,
        });
    }
}

impl ContractDeployment {
    pub fn new(
        network_id: String,
        address: String,
        rpc_url: String,
        proxy_address: Option<String>,
        start_block: Option<u64>,
    ) -> Self {
        Self {
            network_id,
            address,
            rpc_url,
            proxy_address,
            start_block,
        }
    }

    pub fn resolve_network_to_number(&self) -> String {
        // If it's already a number, return as-is
        if let Ok(id) = self.network_id.parse::<u64>() {
            return id.to_string();
        }

        // Look up network ID from supported networks
        for (id, info) in SUPPORTED_NETWORKS.iter() {
            if info.name.to_lowercase() == self.network_id.to_lowercase() {
                return id.to_string();
            }
        }

        // If not found, return original value
        self.network_id.clone()
    }

    pub fn resolve_network_to_string(&self) -> String {
        // If it's not a number, return as-is
        if let Ok(network_id) = self.network_id.parse::<u64>() {
            // Look up network name from supported networks
            if let Some(info) = SUPPORTED_NETWORKS.get(&network_id) {
                return info.name.clone();
            }
        }

        // If not found or not a number, return original value
        self.network_id.clone()
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IndexerConfig {
    pub name: String,
    pub contracts: Vec<ContractConfig>,
}

impl IndexerConfig {
    pub fn new(name: String, contracts: Vec<ContractConfig>) -> Self {
        Self { name, contracts }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("Indexer name cannot be empty".to_string());
        }
        if self.contracts.is_empty() {
            return Err("At least one contract configuration is required".to_string());
        }

        // Validate each contract has at least one deployment
        for contract in &self.contracts {
            if contract.deployments.is_empty() {
                return Err(format!("Contract {} has no deployments", contract.name));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{
        create_test_contract, create_test_explorer_contract, generate_multi_address_contract,
        generate_multi_chain_contract, generate_random_contract_config,
    };

    #[test]
    fn test_single_contract_single_chain() {
        let contract = create_test_contract("SimpleContract", "1");
        let config = IndexerConfig::new("single_test".to_string(), vec![contract]);
        assert!(config.validate().is_ok());
        assert_eq!(
            config.contracts[0].deployments[0].resolve_network_to_number(),
            "1"
        );
    }

    #[test]
    fn test_single_contract_multiple_chains() {
        let contract = generate_multi_chain_contract();
        let config = IndexerConfig::new("multi_chain_test".to_string(), vec![contract]);
        assert!(config.validate().is_ok());

        // Verify first few networks resolve correctly
        assert_eq!(
            config.contracts[0].deployments[0].resolve_network_to_number(),
            "1"
        );
        assert_eq!(
            config.contracts[0].deployments[1].resolve_network_to_number(),
            "10"
        );
        assert_eq!(
            config.contracts[0].deployments[2].resolve_network_to_number(),
            "137"
        );
    }

    #[test]
    fn test_multiple_contracts_same_chain() {
        let contract1 = create_test_contract("Contract1", "1");
        let contract2 = create_test_explorer_contract("Contract2", "1");

        let config = IndexerConfig::new(
            "multi_contract_test".to_string(),
            vec![contract1, contract2],
        );
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_same_contract_multiple_addresses() {
        let contract = generate_multi_address_contract("1", 3);
        let config = IndexerConfig::new("multi_address_test".to_string(), vec![contract]);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_multiple_contracts_different_chains() {
        let contract1 = create_test_contract("EthContract", "1");
        let contract2 = create_test_explorer_contract("OptContract", "10");

        let config = IndexerConfig::new(
            "multi_chain_contract_test".to_string(),
            vec![contract1, contract2],
        );
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_contract_source_validation() {
        // Test random contract configs with different source types
        let contract = generate_random_contract_config();
        assert!(IndexerConfig::new("test".to_string(), vec![contract])
            .validate()
            .is_ok());

        // Test specific contract with ABI source
        let contract = create_test_contract("AbiTest", "1");
        assert!(IndexerConfig::new("test".to_string(), vec![contract])
            .validate()
            .is_ok());

        // Test specific contract with Explorer source
        let contract = create_test_explorer_contract("ExplorerTest", "1");
        assert!(IndexerConfig::new("test".to_string(), vec![contract])
            .validate()
            .is_ok());
    }
}
