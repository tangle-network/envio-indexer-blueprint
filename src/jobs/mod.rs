use crate::{
    kubernetes::{
        envio::{EnvioIndexer, EnvioIndexerConfig, EnvioIndexerSpec},
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
    let result = context
        .spawn_indexer(params.config, params.blockchain, params.rpc_url)
        .await?;
    context.start_indexer(&result.id).await?;

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

    // Create EnvioIndexer CRD
    let indexer = EnvioIndexer {
        metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
            name: Some(params.config.name.clone()),
            namespace: context
                .k8s_manager
                .clone()
                .map(|m| m.namespace().to_string()),
            ..Default::default()
        },
        spec: EnvioIndexerSpec {
            config: EnvioIndexerConfig {
                name: params.config.name,
                abi: params.config.abi,
                blockchain: params.blockchain,
                rpc_url: params.rpc_url,
            },
        },
        status: Some(ServiceStatus {
            phase: ServicePhase::Starting,
            message: Some("Indexer starting".to_string()),
            last_updated: Some(TimeWrapper(Time(chrono::Utc::now()))),
        }),
    };

    // Deploy using K8s manager
    let manager = context
        .k8s_manager
        .ok_or_else(|| "K8s manager not initialized".to_string())?
        .service::<EnvioIndexer>();

    let result = manager.create(&indexer).await.map_err(|e| e.to_string())?;

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
