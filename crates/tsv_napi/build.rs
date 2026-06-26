//! N-API build setup. Configures the linker for the `.node` cdylib (e.g.
//! `-undefined dynamic_lookup` on macOS so the addon resolves Node symbols at
//! load time). A near no-op on Linux, but the idiomatic napi-rs entry point.

fn main() {
    napi_build::setup();
}
