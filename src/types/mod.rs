use alloy_sol_types::SolType;
use serde::{Deserialize, Serialize};

/// Represents a parsed Solidity event parameter
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventParam {
    pub name: String,
    pub param_type: SolidityType,
    pub indexed: bool,
}

/// Represents Solidity types, including complex types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SolidityType {
    // Basic types from alloy
    Address,
    Bool,
    String,
    Bytes(Option<usize>), // None for dynamic bytes, Some(n) for fixed
    Uint(u16),            // e.g., uint256, uint8
    Int(u16),             // e.g., int256, int8
    // Complex types
    Array(Box<SolidityType>, Option<usize>), // Fixed size is Some(size), dynamic is None
    Tuple(Vec<SolidityType>),
    // Custom types (from ABI)
    Custom(String),
}

impl SolidityType {
    /// Convert Solidity type to GraphQL type
    pub fn to_graphql_type(&self) -> Result<String, String> {
        match self {
            SolidityType::Address => Ok("String!".to_string()),
            SolidityType::Bool => Ok("Boolean!".to_string()),
            SolidityType::String => Ok("String!".to_string()),
            SolidityType::Bytes(_) => Ok("String!".to_string()),
            SolidityType::Uint(_) => Ok("BigInt!".to_string()),
            SolidityType::Int(_) => Ok("BigInt!".to_string()),
            SolidityType::Array(inner_type, _) => {
                let inner = inner_type.to_graphql_type()?;
                let inner = inner.trim_end_matches('!');
                Ok(format!("[{}]!", inner))
            }
            SolidityType::Tuple(types) => {
                // For tuples, we create an input type name based on the field types
                let type_names: Vec<_> = types
                    .iter()
                    .map(|t| t.to_graphql_type())
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(format!("Tuple{}!", type_names.join("_")))
            }
            SolidityType::Custom(name) => Ok(format!("{}!", name)),
        }
    }

    /// Convert from alloy SolType
    pub fn from_sol_type<T: SolType>() -> Result<Self, String> {
        match T::SOL_NAME {
            "address" => Ok(SolidityType::Address),
            "bool" => Ok(SolidityType::Bool),
            "string" => Ok(SolidityType::String),
            "bytes" => Ok(SolidityType::Bytes(None)),
            name if name.starts_with("bytes") => {
                let size = name[5..].parse().unwrap_or(0);
                Ok(SolidityType::Bytes(Some(size)))
            }
            name if name.starts_with("uint") => {
                let size = name[4..].parse().unwrap_or(256);
                Ok(SolidityType::Uint(size))
            }
            name if name.starts_with("int") => {
                let size = name[3..].parse().unwrap_or(256);
                Ok(SolidityType::Int(size))
            }
            name if name.ends_with("[]") => {
                let base_type = &name[..name.len() - 2];
                Ok(SolidityType::Array(
                    Box::new(Self::from_type_string(base_type)),
                    None,
                ))
            }
            name if name.contains('[') && name.ends_with(']') => {
                let parts: Vec<_> = name.split('[').collect();
                let base_type = parts[0];
                let size = parts[1].trim_end_matches(']').parse().ok();
                Ok(SolidityType::Array(
                    Box::new(Self::from_type_string(base_type)),
                    size,
                ))
            }
            "tuple" => Ok(SolidityType::Tuple(vec![])), // Simplified for now
            name => Ok(SolidityType::Custom(name.to_string())),
        }
    }

    /// Helper function to convert type string to SolidityType
    pub fn from_type_string(type_str: &str) -> Self {
        match type_str {
            "address" => SolidityType::Address,
            "bool" => SolidityType::Bool,
            "string" => SolidityType::String,
            "bytes" => SolidityType::Bytes(None),
            name if name.starts_with("bytes") => {
                let size = name[5..].parse().unwrap_or(0);
                SolidityType::Bytes(Some(size))
            }
            name if name.starts_with("uint") => {
                let size = name[4..].parse().unwrap_or(256);
                SolidityType::Uint(size)
            }
            name if name.starts_with("int") => {
                let size = name[3..].parse().unwrap_or(256);
                SolidityType::Int(size)
            }
            name if name.ends_with("[]") => {
                let base_type = &name[..name.len() - 2];
                SolidityType::Array(Box::new(Self::from_type_string(base_type)), None)
            }
            name if name.contains('[') && name.ends_with(']') => {
                let parts: Vec<_> = name.split('[').collect();
                let base_type = parts[0];
                let size = parts[1].trim_end_matches(']').parse().ok();
                SolidityType::Array(Box::new(Self::from_type_string(base_type)), size)
            }
            "tuple" => SolidityType::Tuple(vec![]),
            name => SolidityType::Custom(name.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_graphql_type_conversion() {
        let test_cases = vec![
            (SolidityType::Address, "String!"),
            (SolidityType::Bool, "Boolean!"),
            (SolidityType::String, "String!"),
            (SolidityType::Bytes(None), "String!"),
            (SolidityType::Bytes(Some(32)), "String!"),
            (SolidityType::Uint(256), "BigInt!"),
            (SolidityType::Int(256), "BigInt!"),
        ];

        for (sol_type, expected) in test_cases {
            assert_eq!(sol_type.to_graphql_type().unwrap(), expected);
        }
    }

    #[test]
    fn test_array_graphql_type_conversion() {
        let test_cases = vec![
            // Dynamic arrays
            (
                SolidityType::Array(Box::new(SolidityType::Address), None),
                "[String]!",
            ),
            (
                SolidityType::Array(Box::new(SolidityType::Uint(256)), None),
                "[BigInt]!",
            ),
            // Fixed size arrays
            (
                SolidityType::Array(Box::new(SolidityType::Bool), Some(5)),
                "[Boolean]!",
            ),
            // Nested arrays
            (
                SolidityType::Array(
                    Box::new(SolidityType::Array(Box::new(SolidityType::Address), None)),
                    None,
                ),
                "[[String]]!",
            ),
        ];

        for (sol_type, expected) in test_cases {
            assert_eq!(sol_type.to_graphql_type().unwrap(), expected);
        }
    }

    #[test]
    fn test_tuple_graphql_type_conversion() {
        let tuple = SolidityType::Tuple(vec![SolidityType::Address, SolidityType::Uint(256)]);
        assert_eq!(tuple.to_graphql_type().unwrap(), "TupleString!_BigInt!!");

        let nested_tuple = SolidityType::Tuple(vec![
            SolidityType::Address,
            SolidityType::Tuple(vec![SolidityType::Bool, SolidityType::Uint(256)]),
        ]);
        assert_eq!(
            nested_tuple.to_graphql_type().unwrap(),
            "TupleString!_TupleBoolean!_BigInt!!"
        );
    }

    #[test]
    fn test_basic_solidity_type_conversion() {
        let test_cases = vec![
            ("address", SolidityType::Address),
            ("bool", SolidityType::Bool),
            ("string", SolidityType::String),
            ("bytes", SolidityType::Bytes(None)),
            ("bytes32", SolidityType::Bytes(Some(32))),
            ("uint256", SolidityType::Uint(256)),
            ("uint8", SolidityType::Uint(8)),
            ("int256", SolidityType::Int(256)),
            ("int128", SolidityType::Int(128)),
        ];

        for (type_str, expected) in test_cases {
            assert_eq!(SolidityType::from_type_string(type_str), expected);
        }
    }

    #[test]
    fn test_array_solidity_type_conversion() {
        let test_cases = vec![
            (
                "address[]",
                SolidityType::Array(Box::new(SolidityType::Address), None),
            ),
            (
                "uint256[5]",
                SolidityType::Array(Box::new(SolidityType::Uint(256)), Some(5)),
            ),
            (
                "bool[][]",
                SolidityType::Array(
                    Box::new(SolidityType::Array(Box::new(SolidityType::Bool), None)),
                    None,
                ),
            ),
        ];

        for (type_str, expected) in test_cases {
            assert_eq!(SolidityType::from_type_string(type_str), expected);
        }
    }

    #[test]
    fn test_custom_type_conversion() {
        let custom_type = SolidityType::from_type_string("TokenData");
        assert_eq!(custom_type, SolidityType::Custom("TokenData".to_string()));
        assert_eq!(custom_type.to_graphql_type().unwrap(), "TokenData!");
    }

    #[test]
    fn test_event_param_creation() {
        let param = EventParam {
            name: "amount".to_string(),
            param_type: SolidityType::Uint(256),
            indexed: true,
        };

        assert_eq!(param.name, "amount");
        assert_eq!(param.param_type, SolidityType::Uint(256));
        assert!(param.indexed);
    }

    #[test]
    fn test_invalid_size_handling() {
        // Test invalid uint size defaults to 256
        assert_eq!(
            SolidityType::from_type_string("uint999"),
            SolidityType::Uint(256)
        );

        // Test invalid int size defaults to 256
        assert_eq!(
            SolidityType::from_type_string("int999"),
            SolidityType::Int(256)
        );

        // Test invalid bytes size defaults to 0
        assert_eq!(
            SolidityType::from_type_string("bytes999"),
            SolidityType::Bytes(Some(0))
        );
    }
}
