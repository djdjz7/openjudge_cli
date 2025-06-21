#![cfg(test)]
use crate::app::*;
#[test]
fn prefix() {
    assert_eq!(strip_slashes("/123"), "123");
}

#[test]
fn suffix() {
    assert_eq!(strip_slashes("123/"), "123");
}

#[test]
fn both() {
    assert_eq!(strip_slashes("/123/"), "123");
}

#[test]
fn none() {
    assert_eq!(strip_slashes("123"), "123");
}
