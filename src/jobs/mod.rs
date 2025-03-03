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
