use super::parser::parse_event_signature;
use crate::types::EventParam;
use alloy_json_abi::JsonAbi;
use alloy_primitives::hex;
use gadget_sdk::subxt_core::ext::sp_core::keccak_256;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractEvent {
    pub name: String,
    pub signature: String,
    pub inputs: Vec<EventParam>,
}

impl ContractEvent {
    pub fn from_signature(signature: &str, abi: Option<&JsonAbi>) -> Result<Self, String> {
        let (name, params) = parse_event_signature(signature, abi)?;

        Ok(Self {
            name: name.to_string(),
            signature: signature.to_string(),
            inputs: params,
        })
    }

    pub fn get_topic0(&self) -> String {
        format!("0x{}", hex::encode(keccak_256(self.signature.as_bytes())))
    }
}
