use blueprint_sdk::build;
use blueprint_sdk::tangle::blueprint;
use envio_hyperindex_lib::jobs::{spawn_indexer_local, stop_indexer_local};
use std::path::Path;
use std::process;

fn main() {
    // Automatically update dependencies with `soldeer` (if available), and build the contracts.
    //
    // Note that this is provided for convenience, and is not necessary if you wish to handle the
    // contract build step yourself.
    let contract_dirs: Vec<&str> = vec!["../contracts"];
    build::utils::soldeer_install();
    build::utils::soldeer_update();
    build::utils::build_contracts(contract_dirs);

    println!("cargo::rerun-if-changed=../envio-hyperindex-lib");

    // The `blueprint!` macro generates the info necessary for the `blueprint.json`.
    // See its docs for all available metadata fields.
    let blueprint = blueprint! {
        name: "experiment",
        master_manager_revision: "Latest",
        manager: { Evm = "HelloBlueprint" },
        jobs: [spawn_indexer_local, stop_indexer_local]
    };

    match blueprint {
        Ok(blueprint) => {
            // TODO: Should be a helper function probably
            let json = blueprint_sdk::tangle::metadata::macros::ext::serde_json::to_string_pretty(
                &blueprint,
            )
            .unwrap();
            std::fs::write(
                Path::new(env!("CARGO_WORKSPACE_DIR")).join("blueprint.json"),
                json.as_bytes(),
            )
            .unwrap();
        }
        Err(e) => {
            println!("cargo::error={e:?}");
            process::exit(1);
        }
    }
}
