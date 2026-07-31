/**
 * Panic classification — was a per-file failure a tsv CRASH rather than an
 * ordinary rejection?
 *
 * The corpus tools build tsv with `--profile corpus` (release + `panic =
 * "unwind"`) precisely so a panic in our code is CAUGHT and reported instead of
 * killing the run: `tsv_ffi`'s `catch_unwind` renders the payload as the error
 * JSON `{"error": "panic: …"}`, which `lib/ffi.ts` re-throws as an ordinary
 * `Error`. That convenience has a trap attached — the SHIPPED artifacts are
 * built the other way (`release`, `panic = "abort"`), so the same input aborts
 * the host process, and the WASM build reaches its host as a bare
 * `RuntimeError: unreachable`. A caught panic therefore lands in the mildest
 * bucket the run has while describing the harshest failure the release can
 * produce, and it must never grade as "some files errored".
 *
 * Both corpus tools classify the failure MODE before the failure CONTENT:
 * `check_expected_error` keys on the file's *content*, so a panic on a file that
 * also happens to contain SCSS would otherwise be filed as an expected error and
 * dimmed out of the report entirely.
 *
 * Only tsv can produce these shapes — the oracle on the other side of each
 * comparison is JS (prettier, the Svelte/acorn parsers), which throws ordinary
 * `Error`s.
 */

/**
 * Is `message` a caught tsv panic (or WASM trap) rather than a rejection?
 *
 * Matched against the message's FIRST LINE, because a rejection's message can
 * carry a multi-line source code frame quoting the corpus file — content that
 * must never be able to fabricate a panic verdict.
 *
 * Three shapes, one producible today:
 *  - `panic: …` — `tsv_ffi`'s `format_panic` under `catch_unwind`; the only
 *    shape either corpus tool can currently see (both drive the FFI binding).
 *  - `… panicked at …` — Rust's own panic-hook text, for a boundary that
 *    forwards the hook's line rather than the payload (the N-API binding, once
 *    its panic contract lands — 0.4).
 *  - `RuntimeError: unreachable` — how a `panic = "abort"` WASM build's trap
 *    reaches a JS host.
 */
export function is_native_panic_error(message: string | null | undefined): boolean {
	if (!message) return false;
	const first_line = message.split('\n', 1)[0];
	return (
		first_line.startsWith('panic: ') ||
		first_line.includes('panicked at ') ||
		first_line.startsWith('RuntimeError: unreachable')
	);
}
