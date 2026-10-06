// SPDX-License-Identifier: Apache-2.0
use std::{collections::HashSet, error::Error};

use super::{ParseIdError, ProfileId, QueryId, ResultId, SessionId};

macro_rules! id_tests {
    ($module:ident, $id:ident) => {
        mod $module {
            use super::*;

            #[test]
            fn display_parse_round_trip() -> Result<(), Box<dyn Error>> {
                let id = $id::new();
                let text = id.to_string();
                assert_eq!(text.len(), 36);
                assert_eq!(text.parse::<$id>()?, id);
                assert_eq!(uuid::Uuid::parse_str(&text)?.get_version_num(), 7);
                Ok(())
            }

            #[test]
            fn rejects_invalid_input() {
                assert!(matches!(
                    "invalid".parse::<$id>(),
                    Err(ParseIdError::InvalidUuid(_))
                ));
                for input in [
                    "00000000-0000-0000-0000-000000000000",
                    "550e8400-e29b-41d4-a716-446655440000",
                    "01900000-0000-7000-0000-000000000000",
                ] {
                    assert!(matches!(
                        input.parse::<$id>(),
                        Err(ParseIdError::NotVersion7)
                    ));
                    assert!(serde_json::from_str::<$id>(&format!("\"{input}\"")).is_err());
                }
                assert!(serde_json::from_str::<$id>("42").is_err());
            }

            #[test]
            fn serde_json_string_round_trip() -> Result<(), Box<dyn Error>> {
                let id = $id::new();
                let json = serde_json::to_string(&id)?;
                assert_eq!(json, format!("\"{id}\""));
                assert_eq!(serde_json::from_str::<$id>(&json)?, id);
                Ok(())
            }

            #[test]
            fn sequential_ids_are_unique_and_ordered() {
                let ids: Vec<_> = (0..10_000).map(|_| $id::new()).collect();
                assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
                assert_eq!(ids.iter().copied().collect::<HashSet<_>>().len(), ids.len());
            }
        }
    };
}

id_tests!(profiles, ProfileId);
id_tests!(sessions, SessionId);
id_tests!(queries, QueryId);
id_tests!(results, ResultId);
