/**
 * Perf-corpus omit list — the reviewed exceptions to the invariant that every
 * in-scope tool parses/formats every real-world file.
 *
 * The `perf` corpus view (`lib/corpus.ts`) is application, library, and upstream
 * framework source: code that actually ships, so every benchmarked tool is expected
 * to process every file in the languages it declares support for. `bench.ts` enforces
 * that — after the perf pre-flight, a per-file failure that isn't listed here is a
 * hard error, not the silent skip that would quietly erode coverage. (Conformance
 * mode measures coverage, so failures are expected there and the guard doesn't run.)
 *
 * This is the escape hatch for a genuinely-tolerated failure — a third-party tool's
 * bug on a real file, say: list it with a reason so the tolerance stays a reviewed
 * catalogue, never an invisible gap. Keep it EMPTY when it can be; an empty list
 * means every tool handles the whole real-world corpus.
 *
 * Distinct from `parse_sanctions.ts`: those `Sanction`/`KnownGap` lists are about
 * tsv-vs-canonical parse PARITY over the fixture suites (over-rejections tsv keeps
 * or owes), scoped to the correctness gates. This list is about a benchmarked tool
 * FAILING outright on a perf-corpus file, across any tool.
 */

export interface PerfOmit {
	/**
	 * Substring the failing task's `tracking_key` (`operation/language/impl`, e.g.
	 * `parse/svelte/native`) must contain. Omit to tolerate the file across every
	 * task; use a coarse fragment (`svelte/native`) to cover a tool's variants, or
	 * the full key to pin exactly one.
	 */
	task?: string;
	/** Substring the failing file path must contain. */
	path: string;
	/** Why this failure is tolerated — keeps the list a reviewed catalogue, never a silent suppressor. */
	reason: string;
}

/**
 * The reviewed perf-corpus failures. Keep this as close to empty as it can be:
 * the perf corpus is real-world code every in-scope tool should handle. Add an
 * entry only for a deliberately-tolerated failure, each with a reason (see the
 * module doc).
 *
 * The current entries all date from admitting `.d.ts` files to the corpus
 * (which tsv and prettier fully handle) and tolerate third-party limitations
 * on declaration-file-only syntax:
 */
export const PERF_OMITS: PerfOmit[] = [
	// kit's runtime/app/{env,environment}/types.d.ts declare ambient consts with
	// no initializer (`export const browser: boolean;`) — valid ONLY in a
	// declaration file. acorn-typescript has no .d.ts mode at all, and the bench
	// hands oxc/oxfmt a synthetic `file.ts` name (impl calls don't thread the
	// real path), so they grade the content as invalid plain TS. `path:
	// 'src/runtime/app/env'` matches both files (`env/` and `environment/`).
	// Threading real filenames would fix oxc/oxfmt here but not acorn, and would
	// also flip prettier's `.js` parser routing (babel vs typescript) — a
	// measurement-semantics change deliberately not bundled into this tolerance.
	{
		task: 'typescript/canonical',
		path: 'kit/packages/kit/src/runtime/app/env',
		reason: 'acorn-typescript cannot parse ambient const declarations (no .d.ts mode)',
	},
	{
		task: 'typescript/oxc',
		path: 'kit/packages/kit/src/runtime/app/env',
		reason:
			'oxc (native + wasm) rejects ambient consts under the synthetic file.ts name (no path threading in the bench)',
	},
	{
		task: 'typescript/oxfmt',
		path: 'kit/packages/kit/src/runtime/app/env',
		reason:
			'oxfmt rejects ambient consts under the synthetic file.ts name (no path threading in the bench)',
	},
	// acorn-typescript enforces the `arguments`-in-class-field-initializer early
	// error; tsv (permissive / defer-diagnostics policy) and prettier accept it.
	{
		task: 'typescript/canonical',
		path: 'svelte/packages/svelte/src/ambient.d.ts',
		reason: 'acorn-typescript enforces an early error tsv defers (arguments in class field init)',
	},
];

/** First matching omit reason for `(tracking_key, path)`, or `null` when the failure is unlisted. */
export function perf_omit_reason(
	omits: PerfOmit[],
	tracking_key: string,
	path: string,
): string | null {
	return (
		omits.find((o) => (o.task === undefined || tracking_key.includes(o.task)) && path.includes(o.path))
			?.reason ?? null
	);
}
