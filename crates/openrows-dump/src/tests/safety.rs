// SPDX-License-Identifier: Apache-2.0
use super::*;

const SAFE: RestoreRisk = RestoreRisk {
    read_only: false,
    production: false,
    destructive: false,
};

fn confirmed<'a>(token: &'a str, typed: Option<&'a str>) -> Confirmation<'a> {
    Confirmation {
        token: Some(token),
        typed,
    }
}

#[test]
fn read_only_connections_never_restore() {
    let gate = restore_gate(RestoreRisk {
        read_only: true,
        production: true,
        destructive: true,
    });
    assert_eq!(gate, RestoreGate::Blocked);
    let error = admit(gate, "t", "shop", confirmed("t", Some("shop")));
    assert!(error.is_err_and(|error| error.to_string().contains("read-only")));
}

#[test]
fn every_restore_is_confirmed() {
    let gate = restore_gate(SAFE);
    assert_eq!(gate, RestoreGate::Confirm { typed: false });
    let none = Confirmation {
        token: None,
        typed: None,
    };
    assert!(admit(gate, "t", "shop", none).is_err());
    assert!(admit(gate, "t", "shop", confirmed("stale", None)).is_err());
    assert!(admit(gate, "t", "shop", confirmed("t", None)).is_ok());
}

#[test]
fn production_and_destructive_restores_need_the_typed_name() {
    for risk in [
        RestoreRisk {
            production: true,
            ..SAFE
        },
        RestoreRisk {
            destructive: true,
            ..SAFE
        },
    ] {
        let gate = restore_gate(risk);
        assert_eq!(gate, RestoreGate::Confirm { typed: true });
        assert!(admit(gate, "t", "shop", confirmed("t", None)).is_err());
        assert!(admit(gate, "t", "shop", confirmed("t", Some("Shop"))).is_err());
        assert!(admit(gate, "t", "shop", confirmed("t", Some(" shop "))).is_ok());
    }
}

#[test]
fn tokens_change_with_any_part_of_the_restore() {
    let token = confirmation_token(&("p/shop", "/in/a.dump", true));
    assert_eq!(token, confirmation_token(&("p/shop", "/in/a.dump", true)));
    assert_ne!(token, confirmation_token(&("p/shop", "/in/a.dump", false)));
    assert_ne!(token, confirmation_token(&("p/other", "/in/a.dump", true)));
    assert_eq!(token.len(), 16);
}
