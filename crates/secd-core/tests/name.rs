#![allow(non_snake_case)]

use secd_core::{check_name, NameError};

#[test]
fn T_NAME_OK() {
    assert_eq!(check_name("kv/gitea/token"), Ok(()));
}

#[test]
fn T_NAME_DOTDOT() {
    assert_eq!(check_name("kv/../etc/passwd"), Err(NameError::DotDot));
}

#[test]
fn T_NAME_SLASH() {
    assert_eq!(check_name("/abs"), Err(NameError::Slash));
    assert_eq!(check_name("trail/"), Err(NameError::Slash));
    assert!(check_name("").is_err(), "empty name is rejected");
}

#[test]
fn T_NAME_LONG() {
    assert_eq!(check_name(&"a".repeat(256)), Ok(()));
    assert_eq!(check_name(&"a".repeat(257)), Err(NameError::Length));
}

#[test]
fn T_NAME_BAD_CHAR() {
    for name in [
        "has space",
        "kv/gitea/tok en",
        "has\nnewline",
        "kv/gitea/token\n",
        "has\0nul",
        "kv/gitea/token\0",
        " ",
        "\n",
        "\0",
    ] {
        assert_eq!(check_name(name), Err(NameError::BadChar), "{name:?}");
    }
}
