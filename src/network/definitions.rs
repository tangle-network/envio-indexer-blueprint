use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub name: String,
    pub network_id: u64,
    pub rpc_url: String,
    pub supports_traces: bool,
}

// Macro to make network definition more concise and maintainable
macro_rules! define_networks {
    ($(
        $network_id:expr => {
            name: $name:expr,
            rpc: $rpc:expr,
            traces: $traces:expr
        }
    ),* $(,)?) => {
        lazy_static! {
            pub static ref SUPPORTED_NETWORKS: HashMap<u64, NetworkInfo> = {
                let mut m = HashMap::new();
                $(
                    m.insert($network_id, NetworkInfo {
                        name: $name.to_string(),
                        network_id: $network_id,
                        rpc_url: format!("https://{}.hypersync.xyz", $rpc),
                        supports_traces: $traces,
                    });
                )*
                m
            };
        }
    };
}

define_networks! {
  42161 => {
      name: "Arbitrum",
      rpc: "arbitrum",
      traces: false
  },
  42170 => {
      name: "Arbitrum Nova",
      rpc: "arbitrum-nova",
      traces: false
  },
  421614 => {
      name: "Arbitrum Sepolia",
      rpc: "arbitrum-sepolia",
      traces: false
  },
  1313161554 => {
      name: "Aurora",
      rpc: "aurora",
      traces: false
  },
  43114 => {
      name: "Avalanche",
      rpc: "avalanche",
      traces: false
  },
  1123 => {
      name: "B2 Testnet",
      rpc: "b2-testnet",
      traces: false
  },
  8453 => {
      name: "Base",
      rpc: "base",
      traces: false
  },
  84532 => {
      name: "Base Sepolia",
      rpc: "base-sepolia",
      traces: false
  },
  80084 => {
      name: "Berachain Bartio",
      rpc: "berachain-bartio",
      traces: false
  },
  81457 => {
      name: "Blast",
      rpc: "blast",
      traces: false
  },
  168587773 => {
      name: "Blast Sepolia",
      rpc: "blast-sepolia",
      traces: false
  },
  288 => {
      name: "Boba",
      rpc: "boba",
      traces: false
  },
  56 => {
      name: "BSC",
      rpc: "bsc",
      traces: false
  },
  97 => {
      name: "BSC Testnet",
      rpc: "bsc-testnet",
      traces: false
  },
  2001 => {
      name: "C1 Milkomeda",
      rpc: "c1-milkomeda",
      traces: false
  },
  42220 => {
      name: "Celo",
      rpc: "celo",
      traces: false
  },
  8888 => {
      name: "Chiliz",
      rpc: "chiliz",
      traces: false
  },
  5115 => {
      name: "Citrea Testnet",
      rpc: "citrea-testnet",
      traces: false
  },
  44 => {
      name: "Crab",
      rpc: "crab",
      traces: false
  },
  7560 => {
      name: "Cyber",
      rpc: "cyber",
      traces: false
  },
  46 => {
      name: "Darwinia",
      rpc: "darwinia",
      traces: false
  },
  1 => {
      name: "Ethereum Mainnet",
      rpc: "eth",
      traces: true
  },
  250 => {
      name: "Fantom",
      rpc: "fantom",
      traces: false
  },
  14 => {
      name: "Flare",
      rpc: "flare",
      traces: false
  },
  43113 => {
      name: "Fuji",
      rpc: "fuji",
      traces: false
  },
  696969 => {
      name: "Galadriel Devnet",
      rpc: "galadriel-devnet",
      traces: false
  },
  100 => {
      name: "Gnosis",
      rpc: "gnosis",
      traces: true
  },
  10200 => {
      name: "Gnosis Chiado",
      rpc: "gnosis-chiado",
      traces: false
  },
  5 => {
      name: "Goerli",
      rpc: "goerli",
      traces: false
  },
  1666600000 => {
      name: "Harmony Shard 0",
      rpc: "harmony-shard-0",
      traces: false
  },
  17000 => {
      name: "Holesky",
      rpc: "holesky",
      traces: false
  },
  16858666 => {
      name: "Internal Test Chain",
      rpc: "internal-test-chain",
      traces: false
  },
  255 => {
      name: "Kroma",
      rpc: "kroma",
      traces: false
  },
  59144 => {
      name: "Linea",
      rpc: "linea",
      traces: false
  },
  1135 => {
      name: "Lisk",
      rpc: "lisk",
      traces: false
  },
  42 => {
      name: "Lukso",
      rpc: "lukso",
      traces: false
  },
  4201 => {
      name: "Lukso Testnet",
      rpc: "lukso-testnet",
      traces: false
  },
  169 => {
      name: "Manta",
      rpc: "manta",
      traces: false
  },
  5000 => {
      name: "Mantle",
      rpc: "mantle",
      traces: false
  },
  4200 => {
      name: "Merlin",
      rpc: "merlin",
      traces: false
  },
  1088 => {
      name: "Metis",
      rpc: "metis",
      traces: false
  },
  17864 => {
      name: "Mev Commit",
      rpc: "mev-commit",
      traces: false
  },
  34443 => {
      name: "Mode",
      rpc: "mode",
      traces: false
  },
  1287 => {
      name: "Moonbase Alpha",
      rpc: "moonbase-alpha",
      traces: false
  },
  1284 => {
      name: "Moonbeam",
      rpc: "moonbeam",
      traces: false
  },
  2818 => {
      name: "Morph",
      rpc: "morph",
      traces: false
  },
  2810 => {
      name: "Morph Testnet",
      rpc: "morph-testnet",
      traces: false
  },
  245022934 => {
      name: "Neon EVM",
      rpc: "neon-evm",
      traces: false
  },
  204 => {
      name: "opBNB",
      rpc: "opbnb",
      traces: true
  },
  10 => {
      name: "Optimism",
      rpc: "optimism",
      traces: false
  },
  11155420 => {
      name: "Optimism Sepolia",
      rpc: "optimism-sepolia",
      traces: false
  },
  137 => {
      name: "Polygon",
      rpc: "polygon",
      traces: false
  },
  80002 => {
      name: "Polygon Amoy",
      rpc: "polygon-amoy",
      traces: false
  },
  1101 => {
      name: "Polygon zkEVM",
      rpc: "polygon-zkevm",
      traces: false
  },
  30 => {
      name: "Rootstock",
      rpc: "rootstock",
      traces: false
  },
  7225878 => {
      name: "Saakuru",
      rpc: "saakuru",
      traces: false
  },
  534352 => {
      name: "Scroll",
      rpc: "scroll",
      traces: false
  },
  11155111 => {
      name: "Sepolia",
      rpc: "sepolia",
      traces: false
  },
  148 => {
      name: "Shimmer EVM",
      rpc: "shimmer-evm",
      traces: false
  },
  50104 => {
      name: "Sophon",
      rpc: "sophon",
      traces: false
  },
  531050104 => {
      name: "Sophon Testnet",
      rpc: "sophon-testnet",
      traces: false
  },
  5845 => {
      name: "Tangle",
      rpc: "tangle",
      traces: false
  },
  1301 => {
      name: "Unichain Sepolia",
      rpc: "unichain-sepolia",
      traces: false
  },
  196 => {
      name: "X Layer",
      rpc: "x-layer",
      traces: false
  },
  7000 => {
      name: "Zeta",
      rpc: "zeta",
      traces: false
  },
  48900 => {
      name: "Zircuit",
      rpc: "zircuit",
      traces: false
  },
  324 => {
      name: "ZKSync",
      rpc: "zksync",
      traces: false
  },
  7777777 => {
      name: "Zora",
      rpc: "zora",
      traces: false
  },
}
