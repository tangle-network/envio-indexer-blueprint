use crate::{
    envio_utils::IndexerConfig,
    kubernetes::{
        envio::{EnvioIndexer, EnvioIndexerSpec},
        service::{ServicePhase, ServiceStatus, TimeWrapper},
    },
    service_context::SpawnIndexerParams,
};
use gadget_sdk::event_listener::tangle::jobs::services_pre_processor;
use gadget_sdk::event_listener::tangle::TangleEventListener;
use gadget_sdk::job;
use gadget_sdk::tangle_subxt::tangle_testnet_runtime::api::services::events::JobCalled;
use k8s_openapi::{apimachinery::pkg::apis::meta::v1::Time, chrono};

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

    serde_json::to_vec(&result).map_err(|e| format!("Failed to serialize result: {}", e))
}

#[job(
  id = 1,
  params(params),
  event_listener(
      listener = TangleEventListener::<ServiceContext, JobCalled>,
      pre_processor = services_pre_processor,
  ),
)]
pub async fn spawn_indexer_kube(
    params: Vec<u8>,
    context: ServiceContext,
) -> Result<Vec<u8>, String> {
    let params = serde_json::from_slice::<SpawnIndexerParams>(&params)
        .map_err(|e| format!("Failed to parse params: {}", e))?;

    // Validate the configuration
    params.config.validate()?;

    // Create EnvioIndexer CRD for each contract
    let indexers = params
        .config
        .contracts
        .into_iter()
        .map(|contract| EnvioIndexer {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: Some(format!(
                    "{}-{}",
                    params.config.name,
                    contract.name.to_lowercase()
                )),
                namespace: context
                    .k8s_manager
                    .clone()
                    .map(|m| m.namespace().to_string()),
                ..Default::default()
            },
            spec: EnvioIndexerSpec {
                config: IndexerConfig {
                    name: contract.name.clone(),
                    contracts: vec![contract],
                },
            },
            status: Some(ServiceStatus {
                phase: ServicePhase::Starting,
                message: Some("Indexer starting".to_string()),
                last_updated: Some(TimeWrapper(Time(chrono::Utc::now()))),
            }),
        })
        .collect::<Vec<_>>();

    // Deploy using K8s manager
    let manager = context
        .k8s_manager
        .ok_or_else(|| "K8s manager not initialized".to_string())?
        .service::<EnvioIndexer>();

    let mut results = Vec::new();
    for indexer in indexers {
        let result = manager.create(&indexer).await.map_err(|e| e.to_string())?;
        results.push(result);
    }

    serde_json::to_vec(&results).map_err(|e| format!("Failed to serialize results: {}", e))
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

#[job(
    id = 3,
    params(params),
    event_listener(
        listener = TangleEventListener::<ServiceContext, JobCalled>,
        pre_processor = services_pre_processor,
    ),
)]
pub async fn stop_indexer_kube(
    params: Vec<u8>,
    context: ServiceContext,
) -> Result<Vec<u8>, String> {
    let id = String::from_utf8(params).map_err(|e| format!("Failed to parse indexer ID: {}", e))?;

    // Get K8s manager
    let manager = context
        .k8s_manager
        .ok_or_else(|| "K8s manager not initialized".to_string())?
        .service::<EnvioIndexer>();

    // Delete the EnvioIndexer resource
    manager.delete(&id).await.map_err(|e| e.to_string())?;

    Ok(format!("Successfully stopped indexer {}", id).into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        envio_utils::IndexerConfig,
        service_context::{DeploymentMode, SpawnIndexerResult},
        test_utils::{create_test_contract, create_test_explorer_contract},
    };
    use gadget_sdk::config::StdGadgetConfiguration;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_spawn_multi_contract_indexer_local() {
        // Setup test environment
        let context = ServiceContext::new(StdGadgetConfiguration::default(), PathBuf::from("."));

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
    async fn test_kube_spawn_multi_contract_indexer() {
        // Setup test environment
        let mut context =
            ServiceContext::new(StdGadgetConfiguration::default(), PathBuf::from("."));
        context.deployment_mode = DeploymentMode::Kubernetes;
        // Setup mock k8s client here...

        // Create test contracts configuration using test utils
        let contracts = vec![
            create_test_contract("Greeter", "1"), // Ethereum mainnet
            create_test_contract("OptimismGreeter", "10"), // Optimism
        ];

        let config = IndexerConfig::new("multi_network_test".to_string(), contracts);
        let params = SpawnIndexerParams { config };
        let params_bytes = serde_json::to_vec(&params).unwrap();

        let result = spawn_indexer_kube(params_bytes, context).await.unwrap();
        let results: Vec<EnvioIndexer> = serde_json::from_slice(&result).unwrap();

        assert_eq!(results.len(), 3); // One for each contract
        assert!(results[0]
            .metadata
            .name
            .as_ref()
            .unwrap()
            .contains("greeter"));
        assert!(results[1]
            .metadata
            .name
            .as_ref()
            .unwrap()
            .contains("optimismgreeter"));
        assert!(results[2]
            .metadata
            .name
            .as_ref()
            .unwrap()
            .contains("arbitrumtoken"));
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
