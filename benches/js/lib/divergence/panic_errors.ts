/**
 * Panic classification — was a per-file failure a tsv CRASH rather than an
 * ordinary rejection?
 *
 * The corpus tools build tsv with `--profile corpus` (release + `panic =
 * "unwind"`) precisely so a panic in our code is CAUGHT and reported instead of
 * killing the run: both native bindings render the payload as `panic: …`
 * (`tsv_arena::format_panic`) — `tsv_ffi` as the error JSON `{"error": "panic: …"}`,
 * which `lib/ffi.ts` re-throws as an ordinary `Error`, and `tsv_napi` as the message
 * of the `Error` it throws. That convenience has a trap attached — the SHIPPED artifacts are
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
 * Three shapes:
 *  - `panic: …` — a caught panic from either native binding (`tsv_arena::format_panic`:
 *    `tsv_ffi`'s error payload, the message `tsv_napi`'s exports throw), and from
 *    `tsv_debug loc_wires --stdin` (the parse tool's loc arm), whose client prefixes the
 *    payload the same way; the shape both corpus tools see. A panic inside napi-rs's own
 *    argument or return conversion, outside the addon's catch, is thrown by napi-rs's
 *    `catch_unwind` backstop with the bare payload, which no pattern here matches.
 *  - `… panicked at …` — Rust's own panic-hook text, for a boundary that forwards
 *    the hook's line rather than the payload; no tsv binding does.
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

/**
 * The message `tsv_napi`'s test-only `__panic_probe` export throws (the `panic_probe`
 * cargo feature): `panic: ` and the probe's payload. `scripts/test_napi.ts` asserts the
 * built addon throws exactly this and `panic_errors_test.ts` that it classifies, so the
 * addon's text and the classifier read one literal.
 */
export const NAPI_PANIC_PROBE_MESSAGE = 'panic: tsv_napi panic probe';
