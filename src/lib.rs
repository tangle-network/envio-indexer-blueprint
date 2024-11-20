use gadget_sdk::event_listener::tangle::jobs::services_pre_processor;
use gadget_sdk::event_listener::tangle::TangleEventListener;
use gadget_sdk::job;
use gadget_sdk::tangle_subxt::tangle_testnet_runtime::api::services::events::JobCalled;

pub mod envio;
pub mod kubernetes;
pub mod network;
pub mod service_context;
pub mod types;

use k8s_openapi::{apimachinery::pkg::apis::meta::v1::Time, chrono};
use kubernetes::{
    envio::{EnvioIndexer, EnvioIndexerConfig, EnvioIndexerSpec},
    service::{ServicePhase, ServiceStatus, TimeWrapper},
};
use service_context::{DeploymentMode, ServiceContext, SpawnIndexerParams};

#[job(
  id = 0,
  params(params),
  event_listener(
      listener = TangleEventListener::<ServiceContext, JobCalled>,
      pre_processor = services_pre_processor,
  ),
)]
pub async fn spawn_indexer(params: Vec<u8>, context: ServiceContext) -> Result<Vec<u8>, String> {
    let params = serde_json::from_slice::<SpawnIndexerParams>(&params)
        .map_err(|e| format!("Failed to parse params: {}", e))?;

    // Validate the configuration
    params.config.validate()?;

    match context.deployment_mode {
        DeploymentMode::Local => {
            // Use existing EnvioManager implementation
            let result = context
                .spawn_indexer(params.config, params.blockchain, params.rpc_url)
                .await?;
            context.start_indexer(&result.id).await?;
            serde_json::to_vec(&result)
        }
        DeploymentMode::Kubernetes => {
            // Create EnvioIndexer CRD
            let indexer = EnvioIndexerSpec {
                spec: EnvioIndexerConfig {
                    name: params.config.name,
                    abi: params.config.abi,
                    blockchain: params.blockchain,
                    rpc_url: params.rpc_url,
                },
                status: Some(ServiceStatus {
                    phase: ServicePhase::Starting,
                    message: Some("Indexer starting".to_string()),
                    last_updated: Some(TimeWrapper(Time(chrono::Utc::now().into()))),
                }),
            };

            // Deploy using K8s manager
            // In the Kubernetes deployment block
            let manager = context
                .k8s_manager
                .ok_or_else(|| "K8s manager not initialized".to_string())?
                .service::<EnvioIndexerSpec>();

            let result = manager.create(&indexer).await.map_err(|e| e.to_string())?;
            serde_json::to_vec(&result)
        }
    }
    .map_err(|e| format!("Failed to serialize result: {}", e))
}

#[job(
    id = 1,
    params(params),
    event_listener(
        listener = TangleEventListener::<ServiceContext, JobCalled>,
        pre_processor = services_pre_processor,
    ),
)]
pub async fn stop_indexer(params: Vec<u8>, context: ServiceContext) -> Result<Vec<u8>, String> {
    let id = String::from_utf8(params).map_err(|e| format!("Failed to parse indexer ID: {}", e))?;

    context.stop_indexer(&id).await?;

    Ok(format!("Successfully stopped indexer {}", id).into_bytes())
}
