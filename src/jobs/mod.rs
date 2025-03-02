use crate::service_context::SpawnIndexerParams;
use blueprint_sdk::event_listeners::tangle::{
    events::TangleEventListener, services::services_pre_processor,
};
use blueprint_sdk::job;
use blueprint_sdk::tangle_subxt::tangle_testnet_runtime::api::services::events::JobCalled;

use crate::service_context::ServiceContext;

#[job(
  id = 0,
  params(params),
  event_listener(
      listener = TangleEventListener::<ServiceContext, JobCalled>,
      pre_processor = services_pre_processor,
  ),
)]
pub async fn spawn_indexer_local(
    params: Vec<u8>,
    context: ServiceContext,
) -> Result<Vec<u8>, String> {
    let params = serde_json::from_slice::<SpawnIndexerParams>(&params)
        .map_err(|e| format!("Failed to parse params: {}", e))?;

    // Validate the configuration
    params.config.validate()?;

    // Use existing EnvioManager implementation
    let result = context.spawn_indexer(params.config).await?;

    // Start the indexer
    let result = context.start_indexer(&result.id).await?;

    serde_json::to_vec(&result).map_err(|e| format!("Failed to serialize result: {}", e))
}

#[job(
    id = 2,
    params(params),
    event_listener(
        listener = TangleEventListener::<ServiceContext, JobCalled>,
        pre_processor = services_pre_processor,
    ),
)]
pub async fn stop_indexer_local(
    params: Vec<u8>,
    context: ServiceContext,
) -> Result<Vec<u8>, String> {
    let id = String::from_utf8(params).map_err(|e| format!("Failed to parse indexer ID: {}", e))?;

    context.stop_indexer(&id).await?;

    Ok(format!("Successfully stopped indexer {}", id).into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        envio_utils::{ContractConfig, ContractDeployment, ContractSource, IndexerConfig},
        service_context::SpawnIndexerResult,
        test_utils::create_test_contract,
    };
    use blueprint_sdk::{config::GadgetConfiguration, tokio};
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_spawn_multi_contract_indexer_local() {
        // Setup test environment
        let context = ServiceContext::new(GadgetConfiguration::default(), PathBuf::from("."));

        // Create test contracts configuration using test utils
        let contracts = vec![
            create_test_contract("Greeter", "1"), // Ethereum mainnet
            create_test_contract("OptimismGreeter", "10"), // Optimism
        ];

        let config = IndexerConfig::new("multi_network_test".to_string(), contracts);
        let params = SpawnIndexerParams { config };
        let params_bytes = serde_json::to_vec(&params).unwrap();

        // Test local indexer spawn
        let result = spawn_indexer_local(params_bytes, context).await.unwrap();
        let result: SpawnIndexerResult = serde_json::from_slice(&result).unwrap();
        assert!(result.id.contains("multi_network_test"));
    }

    #[tokio::test]
    async fn test_spawn_usdc_indexer() {
        let context = ServiceContext::new(GadgetConfiguration::default(), PathBuf::from("."));

        // USDC contract on Ethereum mainnet
        let usdc_contract = ContractConfig {
            name: "USDC".to_string(),
            source: ContractSource::None,
            deployments: vec![ContractDeployment {
                network_id: "1".to_string(),
                address: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(),
                rpc_url: "https://mainnet.infura.io/v3/".to_string(),
                proxy_address: None,
                start_block: None,
            }],
        };

        let config = IndexerConfig::new("usdc_indexer_test".to_string(), vec![usdc_contract]);
        let params = SpawnIndexerParams { config };
        let params_bytes = serde_json::to_vec(&params).unwrap();

        let result = spawn_indexer_local(params_bytes, context).await.unwrap();
        let result: SpawnIndexerResult = serde_json::from_slice(&result).unwrap();

        assert!(result.id.contains("usdc_indexer_test"));
    }

    #[tokio::test]
    async fn test_indexer_config_validation() {
        // Test empty contracts
        let config = IndexerConfig::new("test".to_string(), vec![]);
        assert!(config.validate().is_err());

        // Test empty name
        let config = IndexerConfig::new("".to_string(), vec![create_test_contract("Test", "1")]);
        assert!(config.validate().is_err());
    }
}
