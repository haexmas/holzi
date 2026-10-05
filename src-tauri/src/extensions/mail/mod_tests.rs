use super::*;

#[test]
fn only_this_device_may_be_reached_without_encryption() {
    for host in ["localhost", "127.0.0.1", "::1", "[::1]", "LOCALHOST"] {
        assert!(
            check_server(host, 1143, ConnectionSecurity::None).is_ok(),
            "{host}"
        );
    }
    for host in ["imap.example.org", "192.168.1.2", "localhost.example.org"] {
        assert!(
            check_server(host, 143, ConnectionSecurity::None).is_err(),
            "{host}"
        );
        assert!(
            check_server(host, 143, ConnectionSecurity::StartTls).is_ok(),
            "{host}"
        );
    }
    assert!(check_server("", 993, ConnectionSecurity::Tls).is_err());
    assert!(check_server("a b", 993, ConnectionSecurity::Tls).is_err());
    assert!(check_server("imap.example.org", 0, ConnectionSecurity::Tls).is_err());
}

#[test]
fn a_flag_is_a_system_flag_or_an_atom_and_never_ends_the_command() {
    for ok in ["\\Seen", "\\Flagged", "$Forwarded", "Junk", "NonJunk"] {
        assert!(check_flag(ok).is_ok(), "{ok}");
    }
    for bad in [
        "",
        "\\",
        "Seen)",
        "(Seen",
        "a b",
        "x\r\nA1 DELETE INBOX",
        "\"quoted\"",
        "{5}",
        "\\Seen\\Deleted",
        "ä",
    ] {
        assert!(check_flag(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn a_mailbox_name_has_no_control_characters() {
    assert!(check_mailbox("INBOX").is_ok());
    assert!(check_mailbox("Gesendete Objekte/2026").is_ok());
    assert!(check_mailbox("").is_err());
    assert!(check_mailbox("INBOX\r\nA1 LOGOUT").is_err());
}

#[test]
fn errors_name_no_secret_and_map_to_bridge_codes() {
    let auth: BridgeError = MailError::Auth.into();
    assert_eq!(auth.code.as_u16(), 2005);
    assert!(!auth.message.contains("password"));
    let large: BridgeError = MailError::TooLarge.into();
    assert_eq!(large.code.as_u16(), 7000);
    let invalid: BridgeError = MailError::Invalid("x".into()).into();
    assert_eq!(invalid.code.as_u16(), 3001);
}
