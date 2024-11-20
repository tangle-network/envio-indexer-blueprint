use crate::types::{EventParam, SolidityType};
use alloy_json_abi::{Event, JsonAbi};
use alloy_sol_types::SolValue;

pub fn parse_event_signature(
    signature: &str,
    abi: Option<&JsonAbi>,
) -> Result<(String, Vec<EventParam>), String> {
    if let Some(abi) = abi {
        if let Some(event) = find_event_in_abi(signature, abi)? {
            return parse_event_from_abi(&event);
        }
    }

    parse_event_from_string(signature)
}

fn find_event_in_abi(signature: &str, abi: &JsonAbi) -> Result<Option<Event>, String> {
    let name = signature
        .split('(')
        .next()
        .ok_or("Invalid event signature format")?;

    Ok(abi.events().find(|e| e.name == name).cloned())
}

fn parse_event_from_abi(event: &Event) -> Result<(String, Vec<EventParam>), String> {
    let params = event
        .inputs
        .iter()
        .map(|param| {
            // Convert the type string to our SolidityType
            let param_type = SolidityType::from_type_string(param.ty.sol_name());

            Ok(EventParam {
                name: param.name.clone(),
                param_type,
                indexed: param.indexed,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok((event.name.clone(), params))
}

fn parse_event_from_string(signature: &str) -> Result<(String, Vec<EventParam>), String> {
    let paren_idx = signature
        .find('(')
        .ok_or("Invalid event signature: missing opening parenthesis")?;

    let name = signature[..paren_idx].trim().to_string();
    if name.is_empty() {
        return Err("Invalid event signature: empty name".to_string());
    }

    let params_str = signature[paren_idx..]
        .trim_matches(|c| c == '(' || c == ')')
        .trim();

    if params_str.is_empty() {
        return Ok((name, Vec::new()));
    }

    let params = params_str
        .split(',')
        .map(|param| parse_param(param.trim()))
        .collect::<Result<Vec<_>, _>>()?;

    Ok((name, params))
}

fn parse_param(param: &str) -> Result<EventParam, String> {
    let parts: Vec<&str> = param.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty parameter".to_string());
    }

    let (param_type, name, indexed) = match parts.len() {
        2 => (parts[0], parts[1], false),
        3 if parts[1] == "indexed" => (parts[0], parts[2], true),
        _ => return Err(format!("Invalid parameter format: {}", param)),
    };

    Ok(EventParam {
        name: name.to_string(),
        param_type: SolidityType::from_type_string(param_type),
        indexed,
    })
}
