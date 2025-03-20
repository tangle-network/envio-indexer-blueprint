use crate::service_context::ServiceContext;
use crate::service_context::SpawnIndexerParams;
use blueprint_sdk::extract::Context;
use blueprint_sdk::tangle::extract::TangleArg;
use blueprint_sdk::tangle::extract::TangleResult;

pub async fn spawn_indexer_local(
    Context(context): Context<ServiceContext>,
    TangleArg(params): TangleArg<SpawnIndexerParams>,
) -> Result<TangleResult<Vec<u8>>, blueprint_sdk::Error> {
    // Validate the configuration
    params
        .config
        .validate()
        .map_err(|e| blueprint_sdk::Error::Other(e))?;

    // Use existing EnvioManager implementation
    let result = context
        .spawn_indexer(params.config)
        .await
        .map_err(|e| blueprint_sdk::Error::Other(e))?;

    // Start the indexer
    let result = context
        .start_indexer(&result.id)
        .await
        .map_err(|e| blueprint_sdk::Error::Other(e))?;

    serde_json::to_vec(&result)
        .map_err(|e| blueprint_sdk::Error::Other(format!("Failed to serialize result: {}", e)))
        .map(TangleResult)
}

pub async fn stop_indexer_local(
    Context(context): Context<ServiceContext>,
    TangleArg(id): TangleArg<String>,
) -> Result<TangleResult<Vec<u8>>, blueprint_sdk::Error> {
    context
        .stop_indexer(&id)
        .await
        .map_err(|e| blueprint_sdk::Error::Other(e))?;

    Ok(TangleResult(
        format!("Successfully stopped indexer {}", id).into_bytes(),
    ))
}
