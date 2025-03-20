use crate::envio_utils::config::{ContractConfig, ContractDeployment, ContractSource};
use crate::envio_utils::IndexerConfig;
use fake::faker::address::en::CountryCode;
use fake::faker::company::en::CompanyName;
use fake::faker::internet::en::DomainSuffix;
use fake::{Fake, Faker};
use rand::seq::SliceRandom;
use rand::Rng;
use tempfile::TempDir;

pub mod erc20_abi;
pub mod greeter_abi;

pub use erc20_abi::ERC20_ABI;
pub use greeter_abi::GREETER_ABI;

// Common network IDs used across test functions
const COMMON_NETWORK_IDS: &[&str] = &["1", "10", "137", "42161", "43114"];

// Random data generation utilities
pub fn generate_random_address() -> String {
    format!("0x{:040x}", rand::random::<u64>())
}

pub fn generate_random_rpc_url() -> String {
    let protocols = ["http", "https", "ws", "wss"];
    let mut rng = rand::thread_rng();
    let protocol = protocols.choose(&mut rng).unwrap();

    format!(
        "{}://{}.{}",
        protocol,
        CompanyName()
            .fake::<String>()
            .to_lowercase()
            .replace(" ", "-"),
        DomainSuffix().fake::<String>()
    )
}

pub fn generate_random_contract_name() -> String {
    format!(
        "{}Contract",
        CompanyName().fake::<String>().replace(" ", "")
    )
}

pub fn generate_random_api_key() -> String {
    format!(
        "{}_{}_key",
        CountryCode().fake::<String>(),
        Faker.fake::<String>()
    )
}

// Contract generation utilities
fn create_deployment(
    network_id: &str,
    address: Option<String>,
    rpc_url: Option<String>,
    proxy_address: Option<String>,
    start_block: Option<u64>,
) -> ContractDeployment {
    ContractDeployment::new(
        network_id.to_string(),
        address.unwrap_or_else(generate_random_address),
        rpc_url.unwrap_or_else(generate_random_rpc_url),
        proxy_address,
        start_block,
    )
}

pub fn create_test_contract(name: &str, network_id: &str) -> ContractConfig {
    ContractConfig::new(
        name.to_string(),
        ContractSource::Abi {
            abi: Some(GREETER_ABI.to_string()),
            url: None,
        },
        vec![create_deployment(
            network_id,
            Some("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".to_string()),
            None,
            None,
            None,
        )],
    )
}

pub fn create_test_explorer_contract(name: &str, network_id: &str) -> ContractConfig {
    ContractConfig::new(
        name.to_string(),
        ContractSource::Explorer {
            api_url: "test_key".to_string(),
        },
        vec![create_deployment(
            network_id,
            Some("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string()),
            None,
            Some("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D".to_string()),
            None,
        )],
    )
}

pub fn generate_random_contract_config() -> ContractConfig {
    let mut rng = rand::thread_rng();
    let network_id = COMMON_NETWORK_IDS.choose(&mut rng).unwrap();

    let source = if rng.gen_bool(0.5) {
        ContractSource::Abi {
            abi: Some(GREETER_ABI.to_string()),
            url: None,
        }
    } else {
        ContractSource::Explorer {
            api_url: generate_random_api_key(),
        }
    };

    let deployments = (0..rng.gen_range(1..=5))
        .map(|_| {
            create_deployment(
                network_id,
                None,
                None,
                if rng.gen_bool(0.3) {
                    Some(generate_random_address())
                } else {
                    None
                },
                None,
            )
        })
        .collect();

    ContractConfig::new(generate_random_contract_name(), source, deployments)
}

pub fn generate_multi_chain_contract() -> ContractConfig {
    ContractConfig::new(
        generate_random_contract_name(),
        ContractSource::Abi {
            abi: Some(GREETER_ABI.to_string()),
            url: None,
        },
        COMMON_NETWORK_IDS
            .iter()
            .map(|&network_id| create_deployment(network_id, None, None, None, None))
            .collect(),
    )
}

pub fn generate_multi_address_contract(network_id: &str, num_addresses: usize) -> ContractConfig {
    ContractConfig::new(
        generate_random_contract_name(),
        ContractSource::Abi {
            abi: Some(GREETER_ABI.to_string()),
            url: None,
        },
        (0..num_addresses)
            .map(|_| create_deployment(network_id, None, None, None, None))
            .collect(),
    )
}

/// Create a USDC contract configuration for testing
pub fn create_usdc_contract() -> IndexerConfig {
    let contract = ContractConfig::new(
        "USDC".to_string(),
        ContractSource::Abi {
            abi: Some(ERC20_ABI.to_string()),
            url: None,
        },
        vec![ContractDeployment::new(
            "1".to_string(), // Ethereum mainnet
            "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
            "https://eth-mainnet.g.alchemy.com/v2/demo".to_string(),
            None,             // No proxy address
            Some(10_000_000), // Start from a more recent block for testing
        )],
    );

    IndexerConfig::new("usdc_indexer_test".to_string(), vec![contract])
}

// File verification utilities
pub fn verify_abi_file(temp_dir: &TempDir, indexer_name: &str, contract_name: &str) -> bool {
    temp_dir
        .path()
        .join(format!("{}_{}_abi.json", indexer_name, contract_name))
        .exists()
}

pub fn read_abi_file(
    temp_dir: &TempDir,
    indexer_name: &str,
    contract_name: &str,
) -> Option<String> {
    std::fs::read_to_string(
        temp_dir
            .path()
            .join(format!("{}_{}_abi.json", indexer_name, contract_name)),
    )
    .ok()
}

pub fn count_abi_files(temp_dir: &TempDir) -> usize {
    std::fs::read_dir(temp_dir.path())
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .unwrap_or_default()
                == "json"
        })
        .count()
}
