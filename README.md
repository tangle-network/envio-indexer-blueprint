# <h1 align="center"> Envio Hyperindexer Blueprint 🌐 </h1>

**A Envio Hyperindexer-as-a-Service Blueprint for Tangle**

## 📚 Overview

A Tangle Blueprint for managing multiple Envio indexers through job execution. This blueprint enables automated deployment and management of Envio indexers through Tangle's job system.

## 🎯 Features

- Spawn multiple Envio indexers per instance
- Manage indexers through Tangle jobs

## 💡 Usage

The blueprint exposes jobs that can be called through Tangle's job system. All configurations are passed as serialized bytes.

### Spawn an Indexer

```rust
// Example job call (from your application)
let config = IndexerConfig {
    name: "uniswap_v3",
    contracts: vec![
        ContractConfig {
            name: "UniswapV3Pool",
            address: "0x...",
            events: vec![...]
            abi: "..."
        }
    ]
};

let params = serde_json::to_vec(&SpawnIndexerParams { config })?;
let result = call_job(0, params).await?; // Returns indexer ID
```

## 📚 Prerequisites

Before you can run this project, you will need to have the following software installed on your machine:

- [Rust](https://www.rust-lang.org/tools/install)
- [Forge](https://getfoundry.sh)
- [Tangle](https://github.com/tangle-network/tangle?tab=readme-ov-file#-getting-started-)
- [Envio](https://envio.dev)

You will also need to install [cargo-tangle](https://crates.io/crates/cargo-tangle), our CLI tool for creating and
deploying Tangle Blueprints:

To install the Tangle CLI, run the following command:

> Supported on Linux, MacOS, and Windows (WSL2)

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/tangle-network/gadget/releases/download/cargo-tangle-v0.1.2/cargo-tangle-installer.sh | sh
```

Or, if you prefer to install the CLI from crates.io:

```bash
cargo install cargo-tangle --force # to get the latest version.
```

## 📜 License

Licensed under either of

- Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license
  ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## 📬 Feedback and Contributions

We welcome feedback and contributions to improve this blueprint.
Please open an issue or submit a pull request on
our [GitHub repository](https://github.com/tangle-network/blueprint-template/issues).

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
