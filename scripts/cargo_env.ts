/**
 * What a cargo child reads from the host. A script's cargo child gets gro's base environment
 * (`PATH`, `HOME`, `TMPDIR`, `TERM`, `LC_ALL=C`, `NO_COLOR`, `FORCE_COLOR`) and the names a
 * root lists. Cargo fingerprints
 * what a build reads, so a build through a script and one through a plain `cargo` npm script
 * find each other's output fresh only when the list holds every variable the build reads:
 * cargo's and rustc's own, and each one a build script in the workspace's graph reruns on
 * (its `cargo:rerun-if-env-changed` lines).
 *
 * @module
 */

/**
 * The host variables a cargo child is given beside gro's base environment:
 *
 * - `CARGO_*` - cargo's own (`CARGO_HOME`, `CARGO_TARGET_DIR`, `CARGO_ENCODED_RUSTFLAGS`,
 *   `CARGO_INCREMENTAL`, `CARGO_TERM_COLOR`, `CARGO_BUILD_JOBS`) and every configuration value
 *   set as one (`CARGO_BUILD_*`, `CARGO_PROFILE_*`, `CARGO_TARGET_*`, `CARGO_HTTP_*`)
 * - `RUSTUP_*` - the toolchain rustup picks (`RUSTUP_TOOLCHAIN`, `RUSTUP_HOME`), beside
 *   `rust-toolchain.toml`
 * - `RUSTC`, `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`, `RUSTFLAGS`, `RUSTDOC`,
 *   `RUSTDOCFLAGS` - the compiler and doc tool cargo runs, what wraps them, and their flags
 * - `RUSTC_BOOTSTRAP`, `RUSTC_STAGE` - read by rustc, and rerun on by `proc-macro2`'s and
 *   `thiserror`'s build scripts
 * - `RUST_BACKTRACE` - what a panicking compiler or build script prints
 * - `RUST_MIN_STACK` - the stack rustc runs its threads on, raised by hand for a crate that
 *   overflows the default
 * - `SCCACHE_*` - the cache a `RUSTC_WRAPPER=sccache` compiles through: where it lives, its
 *   size, its server, its backend
 * - `RUST_LIBC_UNSTABLE_*`, `LIBC_BUILD_VERBOSE` - rerun on by `libc`'s build script
 * - `LIBNODE_PATH`, `TYPE_DEF_TMP_PATH`, `DEBUG_GENERATED_CODE`, `NAPI_*` - rerun on or read
 *   by `napi-build`, which `tsv_napi`'s build script runs
 * - `HTTPS_PROXY`, `https_proxy`, `HTTP_PROXY`, `http_proxy`, `ALL_PROXY`, `all_proxy`,
 *   `NO_PROXY`, `no_proxy` - the proxy cargo's curl fetches a crate it lacks through, and the
 *   hosts it reaches directly; cargo reads the `http`/`https` names itself too, and curl reads
 *   each name in both cases but `HTTP_PROXY`, which only cargo reads
 * - `HTTP_TIMEOUT` - how long cargo waits on a fetch
 * - `SSL_CERT_FILE`, `SSL_CERT_DIR` - the CA certificates the TLS library under cargo's curl
 *   trusts, for a fetch behind a proxy with its own CA
 *
 * Not given, as nothing in a build here reads them: the rest of the host's environment,
 * `CI` and `GITHUB_*` included.
 */
export const CARGO_PASS_ENV: ReadonlyArray<string> = [
	'CARGO_*',
	'RUSTUP_*',
	'RUSTC',
	'RUSTC_WRAPPER',
	'RUSTC_WORKSPACE_WRAPPER',
	'RUSTFLAGS',
	'RUSTDOC',
	'RUSTDOCFLAGS',
	'RUSTC_BOOTSTRAP',
	'RUSTC_STAGE',
	'RUST_BACKTRACE',
	'RUST_MIN_STACK',
	'SCCACHE_*',
	'RUST_LIBC_UNSTABLE_*',
	'LIBC_BUILD_VERBOSE',
	'LIBNODE_PATH',
	'TYPE_DEF_TMP_PATH',
	'DEBUG_GENERATED_CODE',
	'NAPI_*',
	'HTTPS_PROXY',
	'https_proxy',
	'HTTP_PROXY',
	'http_proxy',
	'ALL_PROXY',
	'all_proxy',
	'NO_PROXY',
	'no_proxy',
	'HTTP_TIMEOUT',
	'SSL_CERT_FILE',
	'SSL_CERT_DIR'
];
