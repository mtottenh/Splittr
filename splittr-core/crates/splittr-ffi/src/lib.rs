//! `splittr-ffi` — the FFI surface over the engine (#23).
//!
//! [`Engine`] and the [`dto`] types form the contract that
//! [flutter_rust_bridge](https://cjycode.com/flutter_rust_bridge/) binds into
//! Dart (config: `flutter_rust_bridge.yaml`; generated bindings land in
//! `lib/src/rust/`). The Flutter shell calls these; the engine stays pure Rust.
//!
//! `frb_generated.rs` is the codegen output (`flutter_rust_bridge_codegen
//! generate`); the hand-written API lives in [`api`] + [`dto`].

mod api;
mod convert;
pub mod dto;

#[allow(
    clippy::all,
    dead_code,
    unused_imports,
    non_snake_case,
    clippy::pedantic
)]
mod frb_generated;

pub use api::*;
pub use dto::*;
