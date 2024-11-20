use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NetworkTier {
    #[serde(rename = "🏗️")]
    Development,
    #[serde(rename = "🧪")]
    Experimental,
    #[serde(rename = "🥉")]
    Bronze,
    #[serde(rename = "🥈")]
    Silver,
    #[serde(rename = "🥇")]
    Gold,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub name: String,
    pub network_id: u64,
    pub rpc_url: String,
    pub tier: NetworkTier,
    pub supports_traces: bool,
}

// Macro to make network definition more concise and maintainable
macro_rules! define_networks {
    ($(
        $network_id:expr => {
            name: $name:expr,
            rpc: $rpc:expr,
            tier: $tier:expr,
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
                        tier: $tier,
                        supports_traces: $traces,
                    });
                )*
                m
            };
        }
    };
}

// Define all supported networks
// ... existing code up to define_networks! macro ...

define_networks! {
  42161 => {
      name: "Arbitrum",
      rpc: "arbitrum",
      tier: NetworkTier::Silver,
      traces: false
  },
  42170 => {
      name: "Arbitrum Nova",
      rpc: "arbitrum-nova",
      tier: NetworkTier::Gold,
      traces: false
  },
  421614 => {
      name: "Arbitrum Sepolia",
      rpc: "arbitrum-sepolia",
      tier: NetworkTier::Gold,
      traces: false
  },
  1313161554 => {
      name: "Aurora",
      rpc: "aurora",
      tier: NetworkTier::Silver,
      traces: false
  },
  43114 => {
      name: "Avalanche",
      rpc: "avalanche",
      tier: NetworkTier::Gold,
      traces: false
  },
  1123 => {
      name: "B2 Testnet",
      rpc: "b2-testnet",
      tier: NetworkTier::Development,
      traces: false
  },
  8453 => {
      name: "Base",
      rpc: "base",
      tier: NetworkTier::Silver,
      traces: false
  },
  84532 => {
      name: "Base Sepolia",
      rpc: "base-sepolia",
      tier: NetworkTier::Gold,
      traces: false
  },
  80084 => {
      name: "Berachain Bartio",
      rpc: "berachain-bartio",
      tier: NetworkTier::Silver,
      traces: false
  },
  81457 => {
      name: "Blast",
      rpc: "blast",
      tier: NetworkTier::Silver,
      traces: false
  },
  168587773 => {
      name: "Blast Sepolia",
      rpc: "blast-sepolia",
      tier: NetworkTier::Silver,
      traces: false
  },
  288 => {
      name: "Boba",
      rpc: "boba",
      tier: NetworkTier::Silver,
      traces: false
  },
  56 => {
      name: "BSC",
      rpc: "bsc",
      tier: NetworkTier::Gold,
      traces: false
  },
  97 => {
      name: "BSC Testnet",
      rpc: "bsc-testnet",
      tier: NetworkTier::Experimental,
      traces: false
  },
  2001 => {
      name: "C1 Milkomeda",
      rpc: "c1-milkomeda",
      tier: NetworkTier::Bronze,
      traces: false
  },
  42220 => {
      name: "Celo",
      rpc: "celo",
      tier: NetworkTier::Silver,
      traces: false
  },
  8888 => {
      name: "Chiliz",
      rpc: "chiliz",
      tier: NetworkTier::Silver,
      traces: false
  },
  5115 => {
      name: "Citrea Testnet",
      rpc: "citrea-testnet",
      tier: NetworkTier::Experimental,
      traces: false
  },
  44 => {
      name: "Crab",
      rpc: "crab",
      tier: NetworkTier::Bronze,
      traces: false
  },
  7560 => {
      name: "Cyber",
      rpc: "cyber",
      tier: NetworkTier::Silver,
      traces: false
  },
  46 => {
      name: "Darwinia",
      rpc: "darwinia",
      tier: NetworkTier::Silver,
      traces: false
  },
  1 => {
      name: "Ethereum Mainnet",
      rpc: "eth",
      tier: NetworkTier::Gold,
      traces: true
  },
  250 => {
      name: "Fantom",
      rpc: "fantom",
      tier: NetworkTier::Gold,
      traces: false
  },
  14 => {
      name: "Flare",
      rpc: "flare",
      tier: NetworkTier::Bronze,
      traces: false
  },
  43113 => {
      name: "Fuji",
      rpc: "fuji",
      tier: NetworkTier::Silver,
      traces: false
  },
  696969 => {
      name: "Galadriel Devnet",
      rpc: "galadriel-devnet",
      tier: NetworkTier::Bronze,
      traces: false
  },
  100 => {
      name: "Gnosis",
      rpc: "gnosis",
      tier: NetworkTier::Bronze,
      traces: true
  },
  10200 => {
      name: "Gnosis Chiado",
      rpc: "gnosis-chiado",
      tier: NetworkTier::Bronze,
      traces: false
  },
  5 => {
      name: "Goerli",
      rpc: "goerli",
      tier: NetworkTier::Bronze,
      traces: false
  },
  1666600000 => {
      name: "Harmony Shard 0",
      rpc: "harmony-shard-0",
      tier: NetworkTier::Silver,
      traces: false
  },
  17000 => {
      name: "Holesky",
      rpc: "holesky",
      tier: NetworkTier::Silver,
      traces: false
  },
  16858666 => {
      name: "Internal Test Chain",
      rpc: "internal-test-chain",
      tier: NetworkTier::Development,
      traces: false
  },
  255 => {
      name: "Kroma",
      rpc: "kroma",
      tier: NetworkTier::Bronze,
      traces: false
  },
  59144 => {
      name: "Linea",
      rpc: "linea",
      tier: NetworkTier::Silver,
      traces: false
  },
  1135 => {
      name: "Lisk",
      rpc: "lisk",
      tier: NetworkTier::Silver,
      traces: false
  },
  42 => {
      name: "Lukso",
      rpc: "lukso",
      tier: NetworkTier::Bronze,
      traces: false
  },
  4201 => {
      name: "Lukso Testnet",
      rpc: "lukso-testnet",
      tier: NetworkTier::Bronze,
      traces: false
  },
  169 => {
      name: "Manta",
      rpc: "manta",
      tier: NetworkTier::Silver,
      traces: false
  },
  5000 => {
      name: "Mantle",
      rpc: "mantle",
      tier: NetworkTier::Gold,
      traces: false
  },
  4200 => {
      name: "Merlin",
      rpc: "merlin",
      tier: NetworkTier::Silver,
      traces: false
  },
  1088 => {
      name: "Metis",
      rpc: "metis",
      tier: NetworkTier::Gold,
      traces: false
  },
  17864 => {
      name: "Mev Commit",
      rpc: "mev-commit",
      tier: NetworkTier::Bronze,
      traces: false
  },
  34443 => {
      name: "Mode",
      rpc: "mode",
      tier: NetworkTier::Silver,
      traces: false
  },
  1287 => {
      name: "Moonbase Alpha",
      rpc: "moonbase-alpha",
      tier: NetworkTier::Gold,
      traces: false
  },
  1284 => {
      name: "Moonbeam",
      rpc: "moonbeam",
      tier: NetworkTier::Silver,
      traces: false
  },
  2818 => {
      name: "Morph",
      rpc: "morph",
      tier: NetworkTier::Bronze,
      traces: false
  },
  2810 => {
      name: "Morph Testnet",
      rpc: "morph-testnet",
      tier: NetworkTier::Experimental,
      traces: false
  },
  245022934 => {
      name: "Neon EVM",
      rpc: "neon-evm",
      tier: NetworkTier::Silver,
      traces: false
  },
  204 => {
      name: "opBNB",
      rpc: "opbnb",
      tier: NetworkTier::Silver,
      traces: true
  },
  10 => {
      name: "Optimism",
      rpc: "optimism",
      tier: NetworkTier::Gold,
      traces: false
  },
  11155420 => {
      name: "Optimism Sepolia",
      rpc: "optimism-sepolia",
      tier: NetworkTier::Gold,
      traces: false
  },
  137 => {
      name: "Polygon",
      rpc: "polygon",
      tier: NetworkTier::Gold,
      traces: false
  },
  80002 => {
      name: "Polygon Amoy",
      rpc: "polygon-amoy",
      tier: NetworkTier::Silver,
      traces: false
  },
  1101 => {
      name: "Polygon zkEVM",
      rpc: "polygon-zkevm",
      tier: NetworkTier::Bronze,
      traces: false
  },
  30 => {
      name: "Rootstock",
      rpc: "rootstock",
      tier: NetworkTier::Silver,
      traces: false
  },
  7225878 => {
      name: "Saakuru",
      rpc: "saakuru",
      tier: NetworkTier::Silver,
      traces: false
  },
  534352 => {
      name: "Scroll",
      rpc: "scroll",
      tier: NetworkTier::Silver,
      traces: false
  },
  11155111 => {
      name: "Sepolia",
      rpc: "sepolia",
      tier: NetworkTier::Gold,
      traces: false
  },
  148 => {
      name: "Shimmer EVM",
      rpc: "shimmer-evm",
      tier: NetworkTier::Silver,
      traces: false
  },
  50104 => {
      name: "Sophon",
      rpc: "sophon",
      tier: NetworkTier::Bronze,
      traces: false
  },
  531050104 => {
      name: "Sophon Testnet",
      rpc: "sophon-testnet",
      tier: NetworkTier::Experimental,
      traces: false
  },
  5845 => {
      name: "Tangle",
      rpc: "tangle",
      tier: NetworkTier::Development,
      traces: false
  },
  1301 => {
      name: "Unichain Sepolia",
      rpc: "unichain-sepolia",
      tier: NetworkTier::Development,
      traces: false
  },
  196 => {
      name: "X Layer",
      rpc: "x-layer",
      tier: NetworkTier::Silver,
      traces: false
  },
  7000 => {
      name: "Zeta",
      rpc: "zeta",
      tier: NetworkTier::Gold,
      traces: false
  },
  48900 => {
      name: "Zircuit",
      rpc: "zircuit",
      tier: NetworkTier::Gold,
      traces: false
  },
  324 => {
      name: "ZKSync",
      rpc: "zksync",
      tier: NetworkTier::Gold,
      traces: false
  },
  7777777 => {
      name: "Zora",
      rpc: "zora",
      tier: NetworkTier::Bronze,
      traces: false
  },
}
