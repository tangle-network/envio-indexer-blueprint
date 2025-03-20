use blueprint_sdk::Job;
use blueprint_sdk::Router;
use blueprint_sdk::contexts::tangle::TangleClientContext;
use blueprint_sdk::crypto::sp_core::SpSr25519;
use blueprint_sdk::crypto::tangle_pair_signer::TanglePairSigner;
use blueprint_sdk::keystore::backends::Backend;
use blueprint_sdk::runner::BlueprintRunner;
use blueprint_sdk::runner::config::BlueprintEnvironment;
use blueprint_sdk::runner::tangle::config::TangleConfig;
use blueprint_sdk::tangle::consumer::TangleConsumer;
use blueprint_sdk::tangle::filters::MatchesServiceId;
use blueprint_sdk::tangle::layers::TangleLayer;
use blueprint_sdk::tangle::producer::TangleProducer;
use envio_hyperindex_lib::jobs;
use envio_hyperindex_lib::service_context::ServiceContext;
use tower::filter::FilterLayer;
use tracing::error;
use tracing::level_filters::LevelFilter;

const SPAWN_INDEXER_JOB_ID: u64 = 0;
const STOP_INDEXER_JOB_ID: u64 = 1;

#[tokio::main]
async fn main() -> Result<(), blueprint_sdk::Error> {
    setup_log();

    let env = BlueprintEnvironment::load()?;
    let sr25519_signer = env.keystore().first_local::<SpSr25519>()?;
    let sr25519_pair = env.keystore().get_secret::<SpSr25519>(&sr25519_signer)?;
    let st25519_signer = TanglePairSigner::new(sr25519_pair.0);

    let tangle_client = env.tangle_client().await?;
    let tangle_producer =
        TangleProducer::finalized_blocks(tangle_client.rpc_client.clone()).await?;
    let tangle_consumer = TangleConsumer::new(tangle_client.rpc_client.clone(), st25519_signer);

    let tangle_config = TangleConfig::default();

    let service_id = env.clone().protocol_settings.tangle()?.service_id.unwrap();
    let data_dir = env.clone().data_dir.unwrap().join("data");

    let result = BlueprintRunner::builder(tangle_config, env)
        .router(
            Router::new()
                .route(
                    SPAWN_INDEXER_JOB_ID,
                    jobs::spawn_indexer_local.layer(TangleLayer),
                )
                .route(
                    STOP_INDEXER_JOB_ID,
                    jobs::stop_indexer_local.layer(TangleLayer),
                )
                .layer(FilterLayer::new(MatchesServiceId(service_id)))
                .with_context(ServiceContext::new(
                    BlueprintEnvironment::default(),
                    data_dir,
                )),
        )
        .producer(tangle_producer)
        .consumer(tangle_consumer)
        .with_shutdown_handler(async { println!("Shutting down!") })
        .run()
        .await;

    if let Err(e) = result {
        error!("Runner failed! {e:?}");
    }

    Ok(())
}

pub fn setup_log() {
    use tracing_subscriber::util::SubscriberInitExt;

    let _ = tracing_subscriber::fmt::SubscriberBuilder::default()
        .without_time()
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE)
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .finish()
        .try_init();
}
