use color_eyre::Result;
use envio_hyperindex_blueprint::service_context::ServiceContext;
use gadget_sdk as sdk;
use sdk::runners::tangle::TangleConfig;
use sdk::runners::BlueprintRunner;

#[sdk::main(env)]
async fn main() -> Result<()> {
    let base_dir = env
        .clone()
        .data_dir
        .map(|dir| dir.join("indexers"))
        .unwrap_or_default();
    let context = ServiceContext::new(env.clone(), base_dir);

    tracing::info!("Starting the event watcher ...");
    let tangle_config = TangleConfig::default();
    BlueprintRunner::new(tangle_config, env).run().await?;

    tracing::info!("Exiting...");
    Ok(())
}
