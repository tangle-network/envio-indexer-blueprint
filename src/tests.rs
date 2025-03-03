use super::*;
use crate::{
    envio_utils::{project::IndexerStatus, IndexerConfig},
    jobs::spawn_indexer_local,
    service_context::{ServiceContext, SpawnIndexerParams, SpawnIndexerResult},
    test_utils::{create_test_contract, create_usdc_contract},
};
use blueprint_sdk::{config::GadgetConfiguration, tokio};
use std::{path::PathBuf, time::Duration};

// Add a helper for test cleanup
struct TestCleanup {
    context: ServiceContext,
    indexer_id: Option<String>,
}

impl TestCleanup {
    fn new(context: ServiceContext) -> Self {
        Self {
            context,
            indexer_id: None,
        }
    }

    fn set_indexer_id(&mut self, id: String) {
        self.indexer_id = Some(id);
    }
}

impl Drop for TestCleanup {
    fn drop(&mut self) {
        if let Some(id) = &self.indexer_id {
            // Use block_on to run the async cleanup in a synchronous Drop implementation
            let context = self.context.clone();
            let id = id.clone();
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                println!("Cleaning up test indexer: {}", id);
                // Attempt cleanup - ignore errors since we're in a Drop implementation
                let _ = context.stop_indexer(&id).await;
                // Sleep to allow cleanup to complete
                tokio::time::sleep(Duration::from_secs(1)).await;
            });
        }
    }
}

#[tokio::test]
async fn test_spawn_multi_contract_indexer_local() {
    // Setup test environment
    let context = ServiceContext::new(GadgetConfiguration::default(), PathBuf::from("."));
    let mut cleanup = TestCleanup::new(context.clone());

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

    // Register for cleanup
    cleanup.set_indexer_id(result.id.clone());

    assert!(result.id.contains("multi_network_test"));
}

#[tokio::test]
async fn test_spawn_usdc_indexer() {
    use crate::envio_utils::IndexerLogMessage;

    let context = ServiceContext::new(GadgetConfiguration::default(), PathBuf::from("."));
    let mut cleanup = TestCleanup::new(context.clone());

    let config = test_utils::create_usdc_contract();
    println!("Running USDC indexer test...");

    let params = SpawnIndexerParams { config };
    let params_bytes = serde_json::to_vec(&params).unwrap();

    // Test local indexer spawn
    let result = spawn_indexer_local(params_bytes, context.clone())
        .await
        .unwrap();
    let result: SpawnIndexerResult = serde_json::from_slice(&result).unwrap();

    // Register for cleanup
    cleanup.set_indexer_id(result.id.clone());

    println!("Started indexer: {}", result.id);

    // Subscribe to logs from the indexer
    let mut logs_rx = None;

    // Subscribe to logs using the filtered helper method
    match context.subscribe_to_filtered_logs(&result.id).await {
        Ok(rx) => {
            logs_rx = Some(rx);
            println!("Successfully subscribed to filtered indexer logs");
        }
        Err(e) => {
            println!("Failed to subscribe to logs: {}", e);
        }
    }

    // Process logs in the background
    if let Some(mut rx) = logs_rx {
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                println!("LOG: {}", msg);
            }
        });
    }

    // Monitor the indexer for a short time to verify it's working
    for i in 0..10 {
        let status = context.monitor_indexer(&result.id).await.unwrap();
        let status_str = match status {
            IndexerStatus::Running => "Running".to_string(),
            IndexerStatus::Starting => "Starting".to_string(),
            IndexerStatus::Failed(reason) => format!("Failed: {}", reason),
            IndexerStatus::Configured => "Configured".to_string(),
            IndexerStatus::Stopped => "Stopped".to_string(),
        };

        println!("Cycle {}: Indexer status: {}", i, status_str);

        // Sleep for a bit to allow logs to stream
        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    // Stop explicitly for test visibility (cleanup will handle it if this fails)
    let _ = context.stop_indexer(&result.id).await;
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
