pub mod envio;
pub mod jobs;
pub mod kubernetes;
pub mod network;
pub mod service_context;

#[cfg(test)]
mod tests {
    use super::*;

    use envio::EnvioManager;
    use http::{Request, Response};

    use jobs::spawn_indexer_kube;
    use kube::{api::Api, core::ObjectMeta, Client};
    use kubernetes::{
        envio::{EnvioIndexer, EnvioIndexerConfig, EnvioIndexerSpec},
        K8sManager,
    };
    use service_context::{DeploymentMode, IndexerConfig, ServiceContext, SpawnIndexerParams};
    use std::sync::Arc;
    use std::{collections::HashMap, path::PathBuf};
    use tokio::sync::RwLock;
    use tower_test::mock;

    // Mock types for unit testing
    type MockResponseHandle =
        mock::Handle<Request<kube::client::Body>, Response<kube::client::Body>>;

    /// Helper to create a test context with a mock k8s client
    async fn setup_mock_context() -> (ServiceContext, MockResponseHandle) {
        let (mock_service, handle) = mock::pair();
        let client = Client::new(mock_service, "test-namespace");

        let context = ServiceContext {
            config: gadget_sdk::config::StdGadgetConfiguration::default(),
            indexers: Arc::new(RwLock::new(HashMap::new())),
            envio_manager: Arc::new(EnvioManager::new(PathBuf::from("/tmp"))),
            deployment_mode: DeploymentMode::Kubernetes,
            k8s_manager: Some(K8sManager::new(client, "test-namespace".to_string())),
        };

        (context, handle)
    }

    #[tokio::test]
    async fn test_spawn_indexer_kube_mock() {
        let (context, mut handle) = setup_mock_context().await;

        // Create mock response data
        let response_indexer = EnvioIndexer {
            metadata: ObjectMeta::default(),
            spec: EnvioIndexerSpec {
                config: EnvioIndexerConfig {
                    name: "test-indexer".to_string(),
                    abi: "{}".to_string(),
                    blockchain: "ethereum".to_string(),
                    rpc_url: Some("http://localhost:8545".to_string()),
                },
            },
            status: None,
        };

        // Setup next request/response pair
        let (request, send) = handle.next_request().await.expect("service not called");

        // Verify request
        assert_eq!(request.method(), http::Method::POST);
        assert!(request.uri().path().contains("/envioindexers"));

        // Send response
        let response_bytes = serde_json::to_vec(&response_indexer).unwrap();
        send.send_response(
            Response::builder()
                .status(201)
                .body(response_bytes.into())
                .unwrap(),
        );

        // Test the actual function
        let params = SpawnIndexerParams {
            config: IndexerConfig {
                name: "test-indexer".to_string(),
                abi: r#"{"test": "abi"}"#.to_string(),
            },
            blockchain: "ethereum".to_string(),
            rpc_url: Some("http://localhost:8545".to_string()),
        };

        let params_bytes = serde_json::to_vec(&params).unwrap();
        let result = spawn_indexer_kube(params_bytes, context).await.unwrap();
        let result: EnvioIndexer = serde_json::from_slice(&result).unwrap();

        assert_eq!(result.spec.config.name, "test-indexer");
    }

    // Integration test
    #[tokio::test]
    #[ignore = "requires kubernetes cluster"]
    async fn test_spawn_indexer_kube_integration() {
        let client = Client::try_default().await.unwrap();
        let context = ServiceContext {
            config: gadget_sdk::config::StdGadgetConfiguration::default(),
            indexers: Arc::new(RwLock::new(HashMap::new())),
            envio_manager: Arc::new(EnvioManager::new(PathBuf::from("/tmp"))),
            deployment_mode: DeploymentMode::Kubernetes,
            k8s_manager: Some(K8sManager::new(client, "test-namespace".to_string())),
        };

        let params = SpawnIndexerParams {
            config: IndexerConfig {
                name: "test-indexer".to_string(),
                abi: r#"{"test": "abi"}"#.to_string(),
            },
            blockchain: "ethereum".to_string(),
            rpc_url: Some("http://localhost:8545".to_string()),
        };

        let params_bytes = serde_json::to_vec(&params).unwrap();
        let result = spawn_indexer_kube(params_bytes, context).await.unwrap();
        let result: EnvioIndexer = serde_json::from_slice(&result).unwrap();

        // Verify the indexer was created in kubernetes
        let client = Client::try_default().await.unwrap();
        let indexers: Api<EnvioIndexer> = Api::namespaced(client, "test-namespace");
        let created = indexers.get("test-indexer").await.unwrap();

        assert_eq!(created.spec.config.name, "test-indexer");
    }
}
