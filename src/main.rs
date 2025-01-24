use blueprint_sdk::runners::{core::runner::BlueprintRunner, tangle::tangle::TangleConfig};
use color_eyre::Result;
use envio_hyperindex_blueprint::service_context::ServiceContext;

#[blueprint_sdk::main(env)]
async fn main() -> Result<()> {
    let base_dir = env
        .clone()
        .data_dir
        .map(|dir| dir.join("indexers"))
        .unwrap_or_default();
    let _context = ServiceContext::new(env.clone(), base_dir);

    blueprint_sdk::logging::info!("Starting the event watcher ...");
    let tangle_config = TangleConfig::default();
    BlueprintRunner::new(tangle_config, env).run().await?;

    blueprint_sdk::logging::info!("Exiting...");
    Ok(())
}
