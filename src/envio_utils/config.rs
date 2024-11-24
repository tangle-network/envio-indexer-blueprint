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

    #[test]
    fn test_single_contract_single_chain() {
        let deployment = ContractDeployment::new(
            "Ethereum Mainnet".to_string(),
            "0x123".to_string(),
            "http://eth.local".to_string(),
            None,
            None,
        );

        let contract = ContractConfig::new(
            "SimpleContract".to_string(),
            ContractSource::Abi {
                abi: Some("{}".to_string()),
                url: None,
            },
            vec![deployment],
        );

        let config = IndexerConfig::new("single_test".to_string(), vec![contract]);
        assert!(config.validate().is_ok());
        assert_eq!(
            config.contracts[0].deployments[0].resolve_network_to_number(),
            "1"
        );
    }

    #[test]
    fn test_single_contract_multiple_chains() {
        let deployments = vec![
            ContractDeployment::new(
                "Ethereum Mainnet".to_string(),
                "0x123".to_string(),
                "http://eth.local".to_string(),
                None,
                None,
            ),
            ContractDeployment::new(
                "Optimism".to_string(),
                "0x456".to_string(),
                "http://op.local".to_string(),
                None,
                None,
            ),
            ContractDeployment::new(
                "Arbitrum".to_string(),
                "0x789".to_string(),
                "http://arb.local".to_string(),
                None,
                None,
            ),
        ];

        let contract = ContractConfig::new(
            "MultiChainContract".to_string(),
            ContractSource::Abi {
                abi: None,
                url: Some(
                    "https://api.etherscan.io/api?module=contract&action=getabi&address=0x123"
                        .to_string(),
                ),
            },
            deployments,
        );

        let config = IndexerConfig::new("multi_chain_test".to_string(), vec![contract]);
        assert!(config.validate().is_ok());
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
            "42161"
        );
    }

    #[test]
    fn test_multiple_contracts_same_chain() {
        let deployment1 = ContractDeployment::new(
            "Ethereum Mainnet".to_string(),
            "0x123".to_string(),
            "http://eth.local".to_string(),
            None,
            None,
        );

        let deployment2 = ContractDeployment::new(
            "Ethereum Mainnet".to_string(),
            "0x456".to_string(),
            "http://eth.local".to_string(),
            None,
            None,
        );

        let contracts = vec![
            ContractConfig::new(
                "Contract1".to_string(),
                ContractSource::Abi {
                    abi: Some("{}".to_string()),
                    url: None,
                },
                vec![deployment1],
            ),
            ContractConfig::new(
                "Contract2".to_string(),
                ContractSource::Explorer {
                    api_url: "key123".to_string(),
                },
                vec![deployment2],
            ),
        ];

        let config = IndexerConfig::new("multi_contract_test".to_string(), contracts);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_same_contract_multiple_addresses() {
        let deployments = vec![
            ContractDeployment::new(
                "Ethereum Mainnet".to_string(),
                "0x123".to_string(),
                "http://eth.local".to_string(),
                None,
                None,
            ),
            ContractDeployment::new(
                "Ethereum Mainnet".to_string(),
                "0x456".to_string(),
                "http://eth.local".to_string(),
                None,
                None,
            ),
            ContractDeployment::new(
                "Ethereum Mainnet".to_string(),
                "0x789".to_string(),
                "http://eth.local".to_string(),
                None,
                None,
            ),
        ];

        let contract = ContractConfig::new(
            "MultiAddressContract".to_string(),
            ContractSource::Abi {
                abi: Some("{}".to_string()),
                url: None,
            },
            deployments,
        );

        let config = IndexerConfig::new("multi_address_test".to_string(), vec![contract]);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_multiple_contracts_different_chains() {
        let contracts = vec![
            ContractConfig::new(
                "EthContract".to_string(),
                ContractSource::Abi {
                    abi: Some("{}".to_string()),
                    url: None,
                },
                vec![ContractDeployment::new(
                    "Ethereum Mainnet".to_string(),
                    "0x123".to_string(),
                    "http://eth.local".to_string(),
                    None,
                    None,
                )],
            ),
            ContractConfig::new(
                "OptContract".to_string(),
                ContractSource::Explorer {
                    api_url: "key123".to_string(),
                },
                vec![ContractDeployment::new(
                    "Optimism".to_string(),
                    "0x456".to_string(),
                    "http://op.local".to_string(),
                    Some("0x789".to_string()),
                    None,
                )],
            ),
        ];

        let config = IndexerConfig::new("multi_chain_contract_test".to_string(), contracts);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_contract_source_validation() {
        // Test ABI source with string
        let abi_string = ContractConfig::new(
            "AbiString".to_string(),
            ContractSource::Abi {
                abi: Some("{}".to_string()),
                url: None,
            },
            vec![ContractDeployment::new(
                "1".to_string(),
                "0x123".to_string(),
                "http://local".to_string(),
                None,
                None,
            )],
        );
        assert!(IndexerConfig::new("test".to_string(), vec![abi_string])
            .validate()
            .is_ok());

        // Test ABI source with URL
        let abi_url = ContractConfig::new(
            "AbiUrl".to_string(),
            ContractSource::Abi {
                abi: None,
                url: Some("https://api.example.com/abi".to_string()),
            },
            vec![ContractDeployment::new(
                "1".to_string(),
                "0x123".to_string(),
                "http://local".to_string(),
                None,
                None,
            )],
        );
        assert!(IndexerConfig::new("test".to_string(), vec![abi_url])
            .validate()
            .is_ok());

        // Test Explorer source
        let explorer = ContractConfig::new(
            "Explorer".to_string(),
            ContractSource::Explorer {
                api_url: "key123".to_string(),
            },
            vec![ContractDeployment::new(
                "1".to_string(),
                "0x123".to_string(),
                "http://local".to_string(),
                None,
                None,
            )],
        );
        assert!(IndexerConfig::new("test".to_string(), vec![explorer])
            .validate()
            .is_ok());
    }
}
