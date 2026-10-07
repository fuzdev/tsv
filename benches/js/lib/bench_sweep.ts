/**
 * What the two processes that RUN the implementations share — the pre-flight child
 * (`bench_preflight.ts`), which calls every row once per file to learn what it
 * accepts, and a timed row's child (`bench_row.ts`), which sweeps the accepted set
 * against the clock: the stderr noise filter, the empty-output verdict, and how each
 * of them logs.
 *
 * Kept small and dependency-free: a timed row's process imports it, and that
 * process should hold no more harness than its row needs.
 *
 * @module
 */

import type { Logger, SourceFile } from './types.ts';

/**
 * A child's progress logger. Children inherit the orchestrator's stdio, so a child
 * that printed to stdout would land in the middle of the orchestrator's `--json` /
 * `--markdown` output; the orchestrator says which stream is free.
 */
export const create_logger = (to_stderr: boolean): Logger =>
	to_stderr
		? (...messages) => console.error(...messages)
		: (...messages) => console.log(...messages);

// Several third-party impls write to stderr directly during failure paths,
// bypassing our per-file try/catch:
//
// - `prettier-plugin-svelte`/`prettier-plugin-oxfmt` log via `console.error`
//   inside their babel-parser-fallback chain before re-throwing. The
//   exception is caught and recorded as a skip; the console.error has
//   already flushed.
// - `biome` (WASM) uses `console_error_panic_hook` to write Rust panic
//   text to stderr when an internal AST cast fails. Same shape: panic
//   surfaces through wasm-bindgen as a thrown JS error we catch, but
//   the panic hook has already written.
//
// Skips are already disclosed in the Skipped Files report. The console
// output is pure noise. Filter by substring match against the wrapped
// `console.error`. Patterns are intentionally narrow so unrelated
// errors still surface.
const NOISE_PATTERNS = [
	// oxfmt 0.50 wraps the call site in backticks (`oxfmt::textToDoc()`),
	// so match the unwrapped function name to survive minor wording shifts.
	'oxfmt::textToDoc',
	'panicked at crates/biome_rowan'
];

/**
 * Silence the known third-party stderr noise for the rest of this process, counting
 * what was silenced — see `NOISE_PATTERNS`. Called once, by a process that is about
 * to run implementations, before any of them loads.
 *
 * @returns the live counts by pattern, for the report's `suppressed_noise`
 * @mutates console - wraps `console.error`
 */
export const suppress_stderr_noise = (): Map<string, number> => {
	const original_console_error = console.error.bind(console);
	const suppressed_noise = new Map<string, number>();
	console.error = (...args: unknown[]): void => {
		const probe = args
			.map((a) => (a instanceof Error ? a.message : typeof a === 'string' ? a : ''))
			.join(' ');
		for (const pattern of NOISE_PATTERNS) {
			if (probe.includes(pattern)) {
				suppressed_noise.set(pattern, (suppressed_noise.get(pattern) ?? 0) + 1);
				return;
			}
		}
		original_console_error(...args);
	};
	return suppressed_noise;
};

/**
 * The verdict an impl gives by returning NOTHING: a whitespace-only result for an input
 * that is not is a declined file, not a formatted one, and a timed sweep that accepted it would drop
 * that file's whole cost from the row — on the canonical row, from the denominator of
 * every published `Nx`. The in-process prettier is documented to do exactly this
 * intermittently under load (`CLAUDE.md` §Known Issues), and every other consumer of
 * it guards for it, on the same SEMANTICALLY-empty test as here (`lib/prettier_cache.ts`,
 * `corpus_compare_format.ts`) — a byte-zero test alone misses a tool declining with a bare
 * newline. A `null` return is the `-internal` shape and
 * is not graded. Returns the error to record (pre-flight records it as a skip against
 * the tool) or `null` when the output is present.
 */
export function empty_output_error(
	task_name: string,
	file: SourceFile,
	result: unknown
): Error | null {
	// `/\S/.test` rather than `trim()`: this also runs per file inside the timed loop, and
	// the test stops at the first character of any real output without allocating.
	// It is also the timed loop's only read of a string result, and reading a character is
	// what makes the engine flatten a rope: a formatter that builds its output by
	// concatenation pays that copy inside the clock, as its consumers do. A `.length`
	// check would not, so don't weaken this to one.
	if (typeof result === 'string' && !/\S/.test(result) && /\S/.test(file.content)) {
		return new Error(
			`${task_name} returned empty output for a ${file.bytes}-byte input (${file.path}) — a ` +
				`silently declined file, which would read as zero cost in a timed sweep`
		);
	}
	return null;
}

/** The timed-loop form of `empty_output_error`: throws, so the row errors rather than fake-wins. */
export function assert_output_present(task_name: string, file: SourceFile, result: unknown): void {
	const error = empty_output_error(task_name, file, result);
	if (error !== null) throw error;
}
