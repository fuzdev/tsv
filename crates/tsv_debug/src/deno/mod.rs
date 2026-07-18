//! Embedded Deno sidecar for JS tool access
//!
//! Provides access to prettier, Svelte parser, acorn-typescript parser, and
//! Svelte's CSS parser (parseCss) via a lazily-spawned Deno process. The
//! process is only started when one of these functions is first called.
//!
//! # Example
//!
//! ```ignore
//! use tsv_debug::deno;
//!
//! // Deno is spawned lazily on first call
//! let formatted = deno::run_prettier("<div>hi</div>", "svelte").await?;
//! let ast = deno::parse_svelte("<div>hi</div>").await?;
//! ```

mod actor;
mod error;
mod protocol;

pub use error::DenoError;

use actor::DenoActor;
use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::{OnceCell, RwLock, oneshot};

/// One healable pool slot. The actor is respawned **in place** when a dispatch
/// finds its event loop gone (see [`call_tool`]), so a transient death can't
/// permanently poison the slot's share of the round-robin.
struct ActorSlot {
    actor: RwLock<DenoActor>,
}

/// Global lazy-initialized Deno actor pool
static DENO_POOL: OnceCell<Vec<ActorSlot>> = OnceCell::const_new();

/// Requested pool size, read once when the pool is first spawned
static POOL_SIZE: AtomicUsize = AtomicUsize::new(1);

/// Round-robin dispatch counter
static NEXT_ACTOR: AtomicUsize = AtomicUsize::new(0);

/// Pool size for bulk workloads, given the caller's task concurrency: a few
/// sidecars well below the task count. Each sidecar queues many in-flight
/// requests (the JS side processes them serially — the pool is the only
/// parallelism), and beyond ~3 processes extra sidecars just pay their own
/// module-load cost and memory without improving wall time.
pub fn bulk_pool_size(concurrency: usize) -> usize {
    (concurrency / 4).clamp(1, 4)
}

/// Set the sidecar pool size (number of Deno processes).
///
/// Each sidecar is a single-threaded JS process, so a workload that issues many
/// concurrent calls (fixture validation) is wall-clock-bound on one process;
/// a small pool spreads the JS work across cores. Each process pays its own
/// module-load cost on first use and holds its own memory, so this only pays
/// off for bulk workloads — single-shot commands should leave the default of 1.
///
/// Must be called before the first sidecar call; once the pool is spawned the
/// size is fixed and later calls have no effect.
pub fn set_pool_size(n: usize) {
    POOL_SIZE.store(n.max(1), Ordering::Relaxed);
}

/// Prepare for a bulk workload: derive a task concurrency from the machine's
/// parallelism, size the sidecar pool accordingly (this must happen before the
/// first sidecar call), and return that concurrency so the caller can size its
/// `buffer_unordered` / `buffered` stream. Centralizes the size-pool-then-fan-out
/// ordering that every bulk command needs.
pub fn init_bulk_pool() -> usize {
    let concurrency = std::thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(4);
    set_pool_size(bulk_pool_size(concurrency));
    concurrency
}

/// Get a pool slot, spawning the pool on first use.
///
/// Requests dispatch round-robin: each actor queues concurrent requests
/// (pending-map + per-request oneshot; the JS side processes them serially),
/// so distribution by call count is enough to spread load.
async fn get_slot() -> Result<&'static ActorSlot, DenoError> {
    let pool = DENO_POOL
        .get_or_try_init(|| async {
            (0..POOL_SIZE.load(Ordering::Relaxed))
                .map(|_| {
                    Ok(ActorSlot {
                        actor: RwLock::new(DenoActor::spawn()?),
                    })
                })
                .collect::<Result<Vec<_>, DenoError>>()
        })
        .await?;
    let i = NEXT_ACTOR.fetch_add(1, Ordering::Relaxed) % pool.len();
    Ok(&pool[i])
}

/// Dispatch a tool call to the pool, healing a dead slot in place.
///
/// Healing is **dispatch-level only**: a request that an actor's queue never
/// accepted has provably not reached a sidecar, so completing the dispatch on
/// a fresh actor cannot re-run (and thus cannot mask) failed work. A request
/// that WAS delivered and then failed — crash, timeout, tool error, empty
/// output — fails hard to its caller, deliberately without retry, so a flaky
/// oracle stays loud; the respawn only benefits future calls.
async fn call_tool(tool: &str, content: &str, options: Option<Value>) -> Result<Value, DenoError> {
    let slot = get_slot().await?;

    // Fast path: dispatch on the slot's live actor.
    {
        let actor = slot.actor.read().await;
        if let Some(rx) = actor.dispatch(tool, content, options.as_ref()).await {
            return await_response(rx).await;
        }
    }

    // The actor's event loop is gone (a prior crash/timeout/shutdown killed
    // it). Respawn the slot in place — unless another caller already did while
    // we waited for the write lock — then complete THIS dispatch on the fresh
    // actor (safe: the request never left, see above).
    let mut actor = slot.actor.write().await;
    if actor.is_closed() {
        eprintln!("[deno] respawning dead sidecar actor");
        *actor = DenoActor::spawn()?;
    }
    match actor.dispatch(tool, content, options.as_ref()).await {
        Some(rx) => await_response(rx).await,
        // The fresh actor died before accepting a request — hard error.
        None => Err(DenoError::ActorShutdown),
    }
}

/// Await a dispatched request's response; a dropped sender means the actor's
/// event loop vanished without failing its pending map (e.g. a panic).
async fn await_response(
    rx: oneshot::Receiver<Result<Value, DenoError>>,
) -> Result<Value, DenoError> {
    rx.await.map_err(|_| DenoError::ActorShutdown)?
}

/// Specifies how prettier should determine the parser
#[derive(Debug, Clone, Copy)]
pub enum PrettierParser<'a> {
    /// Explicit parser name (e.g., "svelte", "typescript", "css")
    Parser(&'a str),
    /// Infer parser from filepath extension (e.g., "foo.svelte", "bar.ts")
    Filepath(&'a str),
}

/// Run prettier on content
///
/// # Arguments
/// * `content` - The code to format
/// * `parser` - How to determine the parser (explicit name or infer from filepath)
///
/// # Errors
/// Returns an error if Deno is not available, formatting fails, or prettier
/// returns empty output for non-empty input (the under-load miss — see below).
pub async fn run_prettier(content: &str, parser: PrettierParser<'_>) -> Result<String, DenoError> {
    let options = match parser {
        PrettierParser::Parser(p) => serde_json::json!({ "parser": p }),
        PrettierParser::Filepath(f) => serde_json::json!({ "filepath": f }),
    };

    let result = call_tool("prettier", content, Some(options)).await?;

    let output = result
        .as_str()
        .map(ToString::to_string)
        .ok_or(DenoError::MissingOutput)?;

    // A (semantically) empty format of non-empty input is a prettier
    // malfunction — the documented under-load miss — never a real result.
    // Surface it as a hard, accurately-named error instead of returning
    // fake-empty output that downstream consumers (fixture validation, corpus
    // comparison) would treat as truth. Deliberately NO retry: a flaky oracle
    // must stay loud. Mirrors the corpus compare's own guard
    // (`benches/js/corpus_compare_format.ts`).
    if output.trim().is_empty() && !content.trim().is_empty() {
        return Err(DenoError::EmptyOutput);
    }

    Ok(output)
}

/// Parse Svelte source code using the official Svelte compiler
///
/// # Arguments
/// * `source` - The Svelte source code
///
/// # Returns
/// AST as a JSON Value (caller serializes with desired formatting)
///
/// # Errors
/// Returns an error if Deno is not available or parsing fails.
pub async fn parse_svelte(source: &str) -> Result<Value, DenoError> {
    call_tool("svelte-parse", source, None).await
}

/// The browser-visible render key of Svelte source — the authoritative
/// render-equivalence oracle behind the fixture render-equivalence check.
///
/// Svelte 5 bakes render-time whitespace trimming into the server template at
/// *compile* time but leaves inter-node whitespace runs for the *browser* to
/// collapse, so two authorings that render identically can compile to server JS
/// that differs only in collapsible whitespace. The key reduces the compiled
/// output to its browser-visible render (baked template text, holes for `${…}`,
/// HTML comments stripped, whitespace runs collapsed), so equal keys prove equal
/// renders — and a `<script>`/`<style>` reformatting that leaves the template
/// unchanged yields the same key. See the sidecar's `svelteRenderKey`.
///
/// `compile` runs the full semantic ANALYZER, so it is far stricter than
/// [`parse_svelte`]: it rejects inputs the parser accepts — a TS feature needing a
/// preprocessor, experimental `await`, an illegal default export, a `bind:` to an
/// undeclared or non-assignable target, invalid node placement. Those errors are
/// unrelated to rendering (and `runes: false` does not avoid them). Such a
/// rejection returns [`DenoError::ToolError`]; the render-equivalence check treats
/// it as "compile unavailable" and falls back to the template-only
/// `render_normalize` model.
///
/// # Errors
/// Returns an error if Deno is not available or the compiler rejects the input.
pub async fn svelte_render_key(source: &str) -> Result<String, DenoError> {
    let result = call_tool("svelte-render-key", source, None).await?;
    result
        .as_str()
        .map(ToString::to_string)
        .ok_or(DenoError::MissingOutput)
}

/// Parse TypeScript source code using acorn with TypeScript plugin
///
/// # Arguments
/// * `source` - The TypeScript source code
///
/// # Returns
/// AST as a JSON Value (caller serializes with desired formatting)
///
/// # Errors
/// Returns an error if Deno is not available or parsing fails.
pub async fn parse_typescript(source: &str) -> Result<Value, DenoError> {
    parse_typescript_with_goal(source, tsv_ts::Goal::Module).await
}

/// Parse TypeScript with acorn against an explicit goal symbol.
///
/// `Goal::Module` (the default, via [`parse_typescript`]) mirrors Svelte's
/// always-module parse; `Goal::Script` parses a standalone strict script (acorn
/// `sourceType: 'script'`), so it accepts `await` as an identifier and rejects
/// `import`/`export`/`import.meta` — matching tsv's own `Goal::Script` parse for
/// standalone-script fixtures.
pub async fn parse_typescript_with_goal(
    source: &str,
    goal: tsv_ts::Goal,
) -> Result<Value, DenoError> {
    let options = serde_json::json!({ "sourceType": goal.source_type() });
    call_tool("acorn-typescript-parse", source, Some(options)).await
}

/// Parse CSS source code using Svelte's `parseCss`
///
/// # Arguments
/// * `source` - The CSS source code
///
/// # Returns
/// AST as a JSON Value (caller serializes with desired formatting)
///
/// # Errors
/// Returns an error if Deno is not available or parsing fails.
pub async fn parse_css(source: &str) -> Result<Value, DenoError> {
    call_tool("css-parse", source, None).await
}

/// Parse `content` with the canonical external parser for `parser`
/// (Svelte / acorn-typescript / parseCss). The single dispatch point, so callers
/// keyed on `ParserType` or `InputType::parser_type()` don't re-spell the match.
pub async fn parse_by_type(
    content: &str,
    parser: tsv_cli::cli::input::ParserType,
) -> Result<Value, DenoError> {
    use tsv_cli::cli::input::ParserType;
    match parser {
        ParserType::Svelte => parse_svelte(content).await,
        ParserType::TypeScript => parse_typescript(content).await,
        ParserType::Css => parse_css(content).await,
    }
}

/// Version information from the Deno sidecar
#[derive(Debug, Clone)]
pub struct VersionInfo {
    /// Deno runtime version
    pub deno: String,
    /// TypeScript version (bundled with Deno)
    pub typescript: String,
    /// prettier version
    pub prettier: String,
    /// prettier-plugin-svelte version
    pub prettier_plugin_svelte: String,
    /// svelte compiler version
    pub svelte: String,
    /// acorn parser version
    pub acorn: String,
    /// @sveltejs/acorn-typescript version
    pub acorn_typescript: String,
}

/// Check that Deno sidecar is available and return version info
///
/// This spawns the sidecar if not already running, making it useful for
/// verifying the environment is correctly set up.
///
/// # Errors
/// Returns an error if Deno is not installed or the sidecar fails to start.
pub async fn check() -> Result<VersionInfo, DenoError> {
    let result = call_tool("__version_info", "", None).await?;

    let info = result.as_object().ok_or(DenoError::MissingOutput)?;
    let deps = info
        .get("dependencies")
        .and_then(|d| d.as_object())
        .ok_or(DenoError::MissingOutput)?;

    let get_str = |obj: &serde_json::Map<String, Value>, key: &str| {
        obj.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string()
    };

    Ok(VersionInfo {
        deno: get_str(info, "runtime"),
        typescript: get_str(info, "typescript"),
        prettier: get_str(deps, "prettier"),
        prettier_plugin_svelte: get_str(deps, "prettier-plugin-svelte"),
        svelte: get_str(deps, "svelte"),
        acorn: get_str(deps, "acorn"),
        acorn_typescript: get_str(deps, "@sveltejs/acorn-typescript"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test all deno tools in a single test to avoid race conditions
    /// with the shared static actor across multiple tokio runtimes.
    #[tokio::test]
    async fn test_deno_tools() {
        // Test check (version info)
        let result = check().await;
        assert!(result.is_ok(), "check failed: {result:?}");
        let info = result.unwrap();
        assert!(!info.deno.is_empty());
        assert!(!info.prettier.is_empty());

        // Test prettier
        let result = run_prettier("<div>hello</div>", PrettierParser::Parser("svelte")).await;
        assert!(result.is_ok(), "prettier failed: {result:?}");
        assert_eq!(result.unwrap(), "<div>hello</div>\n");

        // Test svelte parser
        let result = parse_svelte("<div>hello</div>").await;
        assert!(result.is_ok(), "parse_svelte failed: {result:?}");
        let ast = result.unwrap();
        assert_eq!(ast.get("type").and_then(|v| v.as_str()), Some("Root"));

        // Test typescript parser
        let result = parse_typescript("const x: number = 1;").await;
        assert!(result.is_ok(), "parse_typescript failed: {result:?}");
        let ast = result.unwrap();
        assert_eq!(ast.get("type").and_then(|v| v.as_str()), Some("Program"));

        // Test CSS parser
        let result = parse_css(".a { color: red; }").await;
        assert!(result.is_ok(), "parse_css failed: {result:?}");
        let ast = result.unwrap();
        assert_eq!(
            ast.get("type").and_then(|v| v.as_str()),
            Some("StyleSheetFile")
        );

        // Test svelte-render-key — the render-equivalence oracle. The key is the
        // browser-visible render, so render-equivalent authorings share a key
        // while a real content difference does not.
        let flowed = svelte_render_key("<small>a b</small>").await;
        assert!(flowed.is_ok(), "svelte-render-key failed: {flowed:?}");
        let flowed = flowed.unwrap();
        assert!(!flowed.is_empty(), "svelte-render-key produced no output");
        // Block-style boundary whitespace (Svelte trims it at compile) AND a
        // collapsed inter-node run (the browser collapses `a    b` → `a b`) are
        // both render-equivalent to the flowed form.
        let block = svelte_render_key("<small>\n\ta    b\n</small>")
            .await
            .unwrap();
        assert_eq!(
            block, flowed,
            "boundary + collapsible whitespace must share a render key (render-equivalent)"
        );
        // A `<script>` reformatting that leaves the template unchanged shares the key
        // (the key is template-only; script logic is not collected).
        let scripted_a =
            svelte_render_key("<script>\n\tlet x = \"a\";\n</script>\n<small>a b</small>")
                .await
                .unwrap();
        let scripted_b = svelte_render_key("<script>let x = 'a'</script>\n<small>a b</small>")
            .await
            .unwrap();
        assert_eq!(
            scripted_a, scripted_b,
            "a script-only reformatting must not change the render key"
        );
        // A rendered-content difference (`a b` vs `ab`) is NOT render-equivalent.
        let glued = svelte_render_key("<small>ab</small>").await.unwrap();
        assert_ne!(
            glued, flowed,
            "a rendered-content difference must produce a different render key"
        );

        // Pool heal: kill the pooled actor's event loop, then verify the next
        // call finds the dead slot, respawns it in place, and completes the
        // dispatch on the fresh actor. This is the guard against the
        // poisoned-pool failure mode (a dead actor permanently failing its
        // round-robin share). Runs inside this single test because the pool is
        // a process-global static shared across the test binary.
        let slot = get_slot().await.expect("pool must be initialized");
        slot.actor.read().await.shutdown_and_wait().await;
        assert!(
            slot.actor.read().await.is_closed(),
            "actor must be dead before the heal is exercised"
        );
        let result = run_prettier("<div>healed</div>", PrettierParser::Parser("svelte")).await;
        assert!(result.is_ok(), "heal respawn failed: {result:?}");
        assert_eq!(result.unwrap(), "<div>healed</div>\n");
        assert!(
            !slot.actor.read().await.is_closed(),
            "slot must hold a live actor after the heal"
        );
    }

    #[test]
    fn bulk_pool_size_floor_knee_and_ceiling() {
        // Always at least 1, even for degenerate concurrency.
        assert_eq!(bulk_pool_size(0), 1);
        assert_eq!(bulk_pool_size(1), 1);
        // Below the first knee (concurrency/4 < 1) still maps to a single sidecar.
        assert_eq!(bulk_pool_size(3), 1);
        assert_eq!(bulk_pool_size(4), 1);
        assert_eq!(bulk_pool_size(7), 1);
        // Each step of 4 concurrent tasks adds one sidecar.
        assert_eq!(bulk_pool_size(8), 2);
        assert_eq!(bulk_pool_size(12), 3);
        assert_eq!(bulk_pool_size(16), 4);
        // Capped at 4 — extra sidecars only pay module-load cost past the knee.
        assert_eq!(bulk_pool_size(20), 4);
        assert_eq!(bulk_pool_size(128), 4);
    }
}
