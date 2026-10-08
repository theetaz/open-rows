// SPDX-License-Identifier: Apache-2.0
//! Server `ERR` packets as typed driver errors (docs/04 §7): login failures
//! are `Auth`; everything else is `Server` with the packet's SQLSTATE and
//! the `MySQL` error number in the message.
use mysql_common::constants::CapabilityFlags;
use mysql_common::io::ParseBuf;
use mysql_common::packets::ErrPacket;
use openrows_driver_api::DriverError;

/// Error numbers that mean the login was refused.
const AUTH_ERRORS: &[u16] = &[
    1044, // ER_DBACCESS_DENIED_ERROR
    1045, // ER_ACCESS_DENIED_ERROR
    1130, // ER_HOST_NOT_PRIVILEGED
    1251, // ER_NOT_SUPPORTED_AUTH_MODE
    1698, // ER_ACCESS_DENIED_NO_PASSWORD_ERROR
    1862, // ER_MUST_CHANGE_PASSWORD_LOGIN
    3118, // ER_ACCOUNT_HAS_BEEN_LOCKED
    4151, // ER_ACCESS_DENIED_FOR_USER_ACCOUNT_LOCKED (MariaDB)
];

/// A parsed `ERR` packet; progress reports (`MariaDB`) are `None`.
pub(crate) fn err_packet(packet: &[u8], capabilities: CapabilityFlags) -> Option<DriverError> {
    match ParseBuf(packet).parse::<ErrPacket<'_>>(capabilities) {
        Ok(ErrPacket::Error(error)) => {
            let state = error
                .sql_state_ref()
                .map_or_else(|| "HY000".to_owned(), |state| state.as_str().into_owned());
            Some(classify(error.error_code(), state, &error.message_str()))
        }
        Ok(ErrPacket::Progress(_)) => None,
        Err(error) => Some(DriverError::Protocol(format!(
            "malformed ERR packet: {error}"
        ))),
    }
}

/// `Auth` for login refusals, else `Server`.
pub(crate) fn classify(code: u16, state: String, message: &str) -> DriverError {
    if AUTH_ERRORS.contains(&code) {
        return DriverError::Auth(format!("{message} (MySQL error {code})"));
    }
    DriverError::Server {
        code: state,
        message: format!("{message} (MySQL error {code})"),
    }
}

/// A packet the driver did not expect at this point.
pub(crate) fn unexpected(what: &str, packet: &[u8]) -> DriverError {
    let head = packet
        .first()
        .map_or_else(|| "empty".to_owned(), |byte| format!("{byte:#04x}"));
    DriverError::Protocol(format!("unexpected packet ({head}) {what}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err(code: u16, state: &str, message: &str) -> Vec<u8> {
        let mut packet = vec![0xff];
        packet.extend_from_slice(&code.to_le_bytes());
        packet.push(b'#');
        packet.extend_from_slice(state.as_bytes());
        packet.extend_from_slice(message.as_bytes());
        packet
    }

    #[test]
    fn login_refusals_are_auth_errors_and_the_rest_keep_their_sqlstate() {
        let flags = CapabilityFlags::CLIENT_PROTOCOL_41;
        let denied = err_packet(&err(1045, "28000", "Access denied for user 'u'"), flags);
        assert!(matches!(denied, Some(DriverError::Auth(message)) if message.contains("1045")));
        let syntax = err_packet(&err(1064, "42000", "You have an error"), flags);
        assert!(matches!(
            syntax,
            Some(DriverError::Server { code, message }) if code == "42000" && message.contains("(MySQL error 1064)")
        ));
        let duplicate = err_packet(&err(1062, "23000", "Duplicate entry"), flags);
        assert!(matches!(duplicate, Some(DriverError::Server { code, .. }) if code == "23000"));
        assert!(matches!(
            err_packet(&[0xff], flags),
            Some(DriverError::Protocol(_))
        ));
    }
}
