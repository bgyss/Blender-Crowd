//! Emit the macOS extension-module link args for *this* crate.
//!
//! `pyo3-ffi`'s own build script emits `rustc-cdylib-link-arg=-undefined
//! dynamic_lookup`, but Cargo scopes those args to the package whose build
//! script printed them, so they never reach ours. Without this the cdylib
//! fails to link against the CPython symbols it is supposed to resolve from
//! the host interpreter at import time. `maturin` passes the same flags, so
//! this only matters for a bare `cargo build`/`cargo clippy` — which is
//! exactly what CI and contributors run.

fn main() {
    pyo3_build_config::add_extension_module_link_args();
}
