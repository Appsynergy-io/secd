use secd_core::Secret;
use serde::Serialize;
use std::fmt::Display;
use std::ops::Deref;

fn needs_display<T: Display>(_: &T) {}
fn needs_serialize<T: Serialize>(_: &T) {}
fn needs_deref<T: Deref>(_: &T) {}

fn main() {
    let secret = Secret::new(b"printable-secret-bytes".to_vec());
    needs_display(&secret);
    needs_serialize(&secret);
    needs_deref(&secret);
}
