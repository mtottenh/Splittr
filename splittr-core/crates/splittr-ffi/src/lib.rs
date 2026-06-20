//! `splittr-ffi` — the FFI surface over the engine (#23).
//!
//! [`Engine`] and the [`dto`] types form the contract that
//! [flutter_rust_bridge](https://cjycode.com/flutter_rust_bridge/) binds into
//! Dart (config: `flutter_rust_bridge.yaml`; generated bindings land in
//! `lib/src/rust/`). The Flutter shell calls these; the engine stays pure Rust.
//!
//! This crate is plain, FRB-friendly Rust and is unit-tested directly — running
//! codegen + native bundling is a build step (CI/local), not a runtime concern.

mod api;
mod convert;
pub mod dto;

pub use api::Engine;
pub use dto::*;
