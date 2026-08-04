// The whole crate lives here, so `main.rs` is a shim over `cli` and every module —
// including the two the binary alone used to own, `audit` and `cli` — is reachable by
// the integration tests in `tests/` and documented by `cargo doc`.
pub mod audit;
pub mod cli;
pub mod compile_fixtures;
pub mod deno;
pub mod diff;
pub mod error;
pub mod fixtures;
pub mod render_browser;
pub mod render_normalize;
pub mod test262;
pub mod tsc_conformance;
