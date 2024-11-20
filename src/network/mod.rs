pub mod definitions;
pub use definitions::{NetworkInfo, NetworkTier, SUPPORTED_NETWORKS};

/// Validates if a network ID is supported and returns its information
pub fn validate_network(network_id: u64) -> Result<&'static NetworkInfo, String> {
    SUPPORTED_NETWORKS
        .get(&network_id)
        .ok_or_else(|| format!("Unsupported network ID: {}", network_id))
}

/// Returns all supported network IDs
pub fn supported_network_ids() -> Vec<u64> {
    SUPPORTED_NETWORKS.keys().cloned().collect()
}

/// Returns all networks that support traces
pub fn networks_with_traces() -> Vec<&'static NetworkInfo> {
    SUPPORTED_NETWORKS
        .values()
        .filter(|network| network.supports_traces)
        .collect()
}

/// Returns networks by tier
pub fn networks_by_tier(tier: NetworkTier) -> Vec<&'static NetworkInfo> {
    SUPPORTED_NETWORKS
        .values()
        .filter(|network| network.tier == tier)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_validation() {
        // Test valid network
        assert!(validate_network(1).is_ok());

        // Test invalid network
        assert!(validate_network(999999).is_err());
    }

    #[test]
    fn test_networks_with_traces() {
        let trace_networks = networks_with_traces();
        assert!(trace_networks.iter().any(|n| n.network_id == 1)); // Ethereum should support traces
    }

    #[test]
    fn test_networks_by_tier() {
        let gold_networks = networks_by_tier(NetworkTier::Gold);
        assert!(!gold_networks.is_empty());
        assert!(gold_networks.iter().all(|n| n.tier == NetworkTier::Gold));
    }
}
