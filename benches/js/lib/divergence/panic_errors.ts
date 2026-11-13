/**
 * Crash classification — was a per-file failure a tsv CRASH rather than an
 * ordinary rejection?
 *
 * The corpus tools build tsv's native engines with `--profile corpus` (release + `panic =
 * "unwind"`) precisely so a panic in our code is CAUGHT and reported instead of
 * killing the run: the N-API addon throws it as an `Error` whose message is `panic: …`
 * (`tsv_arena::format_panic`), and the `tsv_debug` loc wire's client the same. That
 * convenience has a trap attached — the SHIPPED artifacts are built the other way
 * (`release`, `panic = "abort"`), so the same input aborts the host process. The WASM
 * bundles are built that way too, so their panic reaches the host as a trap, a
 * `WebAssembly.RuntimeError`. A caught panic or a trap therefore lands in the mildest bucket
 * the run has while describing the harshest failure the release can produce, and it must
 * never grade as "some files errored" or as a rejection.
 *
 * Every tool that grades tsv classifies the failure MODE before the failure CONTENT:
 * `check_expected_error` keys on the file's *content*, so a panic on a file that also
 * happens to contain SCSS would otherwise be filed as an expected error and dimmed out of
 * the report entirely.
 *
 * The classification reads what a `catch` around a tsv call caught, by its type, never a
 * message alone: a rejection's text is tsv's error message, which quotes the source (a
 * parse error can name an `unreachable` identifier), so no pattern over text that a
 * rejection can carry may decide a crash. Each binding's wrapper throws the failures the
 * engine RETURNED as a `TsvRejection`, which is never a crash, whatever its message says.
 */

import { first_line } from '../error_text.ts';

/**
 * A failure tsv's engine RETURNED in its product's place — a parse failure, a refusal of the
 * source — thrown by the harness as the row's rejection: `TsvBinding`'s calls over the
 * N-API and WASM engines (`lib/tsv_api.ts`) and the loc wire's `{error}` reply
 * (`lib/loc_wire_client.ts`). Its own class so `is_tsv_crash` can tell it from what the
 * engine THREW: a returned failure is a verdict on the source, whatever its text says.
 */
export class TsvRejection extends Error {}

/**
 * Is `thrown` — what a `catch` around a tsv call caught — a crash rather than a rejection?
 * By the thrown value's type:
 *  - a `TsvRejection` never is: the engine returned it (see the class).
 *  - a `WebAssembly.RuntimeError` always is: it is a trap, the module executing what the
 *    engine refuses to run (`unreachable`, which a `panic = "abort"` build's panic
 *    compiles to; an out-of-bounds memory access; an indirect call through a null or
 *    mistyped entry; an integer division by zero or overflow), each a bug in tsv, whatever
 *    its message.
 *  - a plain `Error` (no subclass) whose message's FIRST LINE starts `panic: ` is a caught
 *    native panic: the N-API addon throws exactly that (napi-rs's `Error`, which
 *    `scripts/test_napi.ts` holds to), and so does the loc wire's client. The first line,
 *    so a code frame can't fabricate one. A panic inside napi-rs's own argument or return
 *    conversion, outside the addon's catch, is thrown by napi-rs's `catch_unwind` backstop
 *    with the bare payload, which this does not recognize.
 *  - anything else is not: a `TypeError` is an argument refusal, even one whose text reads
 *    `panic: `; a `RangeError` (the host's stack exhausted, a string past its length) is a
 *    host limit that any JS frame between the call and the catch can raise, prettier's and
 *    the harness's included, so its type can't name tsv; a value that is no `Error` comes
 *    from no tsv binding.
 *
 * The C FFI's wrapper (`lib/ffi.ts`, the Deno bench row's) throws every failure itself, as
 * a plain `Error`, so the bench's pre-flight and `facade_probe` read its throws by text alone.
 * That holds because a returned message never begins `panic: ` (each starts with fixed text):
 * the release FFI aborts on a panic, and an unwinding one (`TSV_FFI_PROFILE`, which
 * `facade_probe` honors) reports it as `panic: …`, a crash.
 */
export function is_tsv_crash(thrown: unknown): boolean {
	if (thrown instanceof WebAssembly.RuntimeError) return true;
	// a plain `Error`, never a subclass: `TsvRejection` is the returned failure, a `TypeError` the
	// facade's argument refusal
	if (!(thrown instanceof Error) || Object.getPrototypeOf(thrown) !== Error.prototype) return false;
	return thrown.message.split('\n', 1)[0].startsWith('panic: ');
}

/**
 * A crash's text for its FAIL entry: the first line of its message, a trap's prefixed with
 * its type, since a trap's message alone (`unreachable`) doesn't say what happened.
 */
export function crash_text(thrown: unknown): string {
	const text = first_line(thrown);
	return thrown instanceof WebAssembly.RuntimeError ? `RuntimeError: ${text}` : text;
}

/** A crash as a FAIL block lists it: where it happened, and what tsv threw (`crash_text`). */
export interface PanicEntry {
	/** The file, and for a tool grading several tsv engines, the engine. */
	label: string;
	error: string;
}

/** How many panics a FAIL block lists; a tool's JSON report, where it has one, carries all. */
const PANICS_LISTED = 10;

/**
 * The FAIL block every tool that grades tsv prints on a crash, uncolored, one line
 * per element: the headline, `  <label>: <error>` for the first ten, then a count of the
 * rest. One spelling, so a panic reads the same in every tool's report, whichever stream
 * the tool keeps for it.
 */
export function panic_report_lines(panics: readonly PanicEntry[]): string[] {
	const lines = [
		`FAIL: ${panics.length} panic(s) — tsv crashed on real code (a native panic the corpus ` +
			`profile caught, which a shipped artifact aborts the host on, or a WASM trap):`
	];
	for (const p of panics.slice(0, PANICS_LISTED)) lines.push(`  ${p.label}: ${p.error}`);
	if (panics.length > PANICS_LISTED) lines.push(`  ... and ${panics.length - PANICS_LISTED} more`);
	return lines;
}

/**
 * The message `tsv_napi`'s test-only `__panic_probe` export throws (the `panic_probe`
 * cargo feature): `panic: ` and the probe's payload. `scripts/test_napi.ts` asserts the
 * built addon throws exactly this and `panic_errors_test.ts` that it classifies, so the
 * addon's text and the classifier read one literal.
 */
export const NAPI_PANIC_PROBE_MESSAGE = 'panic: tsv_napi panic probe';
