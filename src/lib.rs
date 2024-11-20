use gadget_sdk::event_listener::tangle::jobs::{services_post_processor, services_pre_processor};
use gadget_sdk::event_listener::tangle::TangleEventListener;
use gadget_sdk::job;
use gadget_sdk::tangle_subxt::tangle_testnet_runtime::api::services::events::JobCalled;

pub mod envio;
pub mod generator;
pub mod indexer_utils;
pub mod network;
pub mod service_context;
pub mod types;

use service_context::{ServiceContext, SpawnIndexerParams};

#[job(
  id = 0,
  params(params),
  event_listener(
      listener = TangleEventListener::<ServiceContext, JobCalled>,
      pre_processor = services_pre_processor,
      post_processor = services_post_processor,
  ),
)]
pub async fn spawn_indexer(params: Vec<u8>, context: ServiceContext) -> Result<Vec<u8>, String> {
    let params = serde_json::from_slice::<SpawnIndexerParams>(&params)
        .map_err(|e| format!("Failed to parse params: {}", e))?;

    // Validate the configuration
    params.config.validate()?;

    // Register and start the indexer
    let result = context.spawn_indexer(params.config).await?;
    context.start_indexer(&result.id).await?;

    // Serialize the result
    serde_json::to_vec(&result).map_err(|e| format!("Failed to serialize result: {}", e))
}
