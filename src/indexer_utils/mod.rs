mod config;
mod event;
mod parser;

pub use config::{ContractConfig, IndexerConfig};
pub use event::ContractEvent;

#[cfg(test)]
mod tests {
    use crate::types::SolidityType;

    use super::*;

    #[test]
    fn test_parse_transfer_event() {
        let sig = "Transfer(address indexed from, address indexed to, uint256 value)";
        let event = ContractEvent::from_signature(sig, None).unwrap();

        assert_eq!(event.name, "Transfer");
        assert_eq!(event.inputs.len(), 3);

        assert_eq!(event.inputs[0].name, "from");
        assert!(matches!(event.inputs[0].param_type, SolidityType::Address));
        assert!(event.inputs[0].indexed);

        assert_eq!(event.inputs[2].name, "value");
        assert!(matches!(
            event.inputs[2].param_type,
            SolidityType::Uint(256)
        ));
        assert!(!event.inputs[2].indexed);
    }

    #[test]
    fn test_invalid_signature() {
        let sig = "Transfer(invalid type param)";
        assert!(ContractEvent::from_signature(sig, None).is_err());
    }
}
