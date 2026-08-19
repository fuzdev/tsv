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
 * A RATCHET, not an accumulator: a full perf run grades the list in both
 * directions — an unlisted failure fails, and so does an entry that excused
 * nothing on a run that could have exercised it (`stale_perf_omits`). So a
 * tolerance cannot outlive the failure it was written for. The entries must also
 * be DISJOINT, which the caller enforces on what a run observed: a failure two
 * entries both claim fails the run (`perf_omit_matches`). It can still be written
 * too BROADLY without overlapping anything, which no grading catches — see
 * `stale_perf_omits`.
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
	 * the full key to pin exactly one. Include the operation when the impl key is
	 * shared across operations — `canonical` is both the parse (acorn/svelte) and
	 * format (prettier) key, so a bare `typescript/canonical` would excuse either.
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
		task: 'parse/typescript/canonical',
		path: 'kit/packages/kit/src/runtime/app/env',
		reason: 'acorn-typescript cannot parse ambient const declarations (no .d.ts mode)'
	},
	{
		task: 'parse/typescript/oxc',
		path: 'kit/packages/kit/src/runtime/app/env',
		reason:
			'oxc (native + wasm) rejects ambient consts under the synthetic file.ts name (no path threading in the bench)'
	},
	{
		task: 'format/typescript/oxfmt',
		path: 'kit/packages/kit/src/runtime/app/env',
		reason:
			'oxfmt rejects ambient consts under the synthetic file.ts name (no path threading in the bench)'
	},
	// Same two files, same cause, one tool later: yuku (native + wasm) rejects the
	// ambient consts. It differs from oxc in having an explicit `lang: 'dts'` mode
	// that would accept them — but selecting it needs the real path, the same
	// threading the entries above decline, so the tolerance stays uniform across
	// the alternative parsers rather than special-casing one of them.
	{
		task: 'parse/typescript/yuku',
		path: 'kit/packages/kit/src/runtime/app/env',
		reason:
			'yuku (native + wasm) rejects ambient consts under the pinned `lang: ts` (its `dts` mode needs path threading the bench does not do)'
	},
	// acorn-typescript enforces the `arguments`-in-class-field-initializer early
	// error; tsv (permissive / defer-diagnostics policy) and prettier accept it.
	{
		task: 'parse/typescript/canonical',
		path: 'svelte/packages/svelte/src/ambient.d.ts',
		reason: 'acorn-typescript enforces an early error tsv defers (arguments in class field init)'
	},
	// swc on the same two declaration-file shapes as the entries above. It differs
	// from oxc and yuku in WHY: for those, the bench's synthetic `file.ts` name (or a
	// pinned `lang: ts`) is what withholds declaration-file mode, so the tolerance is
	// really about path threading. swc rejects these with `dts: true` passed
	// EXPLICITLY — verified — so this is the parser's own limit, not a harness
	// artifact, and no amount of path threading would change it.
	{
		task: 'parse/typescript/swc',
		path: 'kit/packages/kit/src/runtime/app/env',
		reason: 'swc rejects ambient const declarations even with its own `dts` mode enabled'
	},
	{
		task: 'parse/typescript/swc',
		path: 'svelte/packages/svelte/src/ambient.d.ts',
		reason:
			'swc enforces the strict-mode eval/arguments binding early error tsv defers (`export const arguments: never`)'
	}
];

/**
 * EVERY omit excusing `(tracking_key, path)`, in list order — empty when the
 * failure is unlisted.
 *
 * Returns the ENTRIES, not their reasons, because the caller has three questions
 * and the reason answers only one: "is this failure excused?" (any match at all),
 * "did this excuse fire?" (the ratchet's second direction — see
 * `stale_perf_omits`), and "does more than one entry claim it?".
 *
 * ALL matches rather than the first, and that third question is why. Under a
 * first-match rule a broad entry SHADOWS a narrower one that also applies: only
 * the broad one is marked used, so the shadowed entry then reports as STALE while
 * the failure it describes is live — the exact inverse of what happened. Crediting
 * every match makes that misreport unreachable and leaves the overlap itself
 * visible, which is the defect actually worth naming: two entries claiming one
 * failure means neither is the entry that describes it, and one of them is
 * redundant or too broad. The caller fails the run on that
 * (`enforce_perf_coverage`).
 *
 * Disjointness is checkable only against OBSERVED failures, never structurally:
 * both predicates are substring tests, so for any two entries some string contains
 * both fragments and no pair is disjoint in the abstract. What a run sees is the
 * answerable question, and it is also the only one that matters.
 */
export function perf_omit_matches(
	omits: readonly PerfOmit[],
	tracking_key: string,
	path: string
): PerfOmit[] {
	return omits.filter(
		(o) => (o.task === undefined || tracking_key.includes(o.task)) && path.includes(o.path)
	);
}

/**
 * The entries in `omits` that `used` never matched, restricted to the ones a run
 * over `graded_keys` could actually have exercised — a tolerance for a failure
 * that no longer happens.
 *
 * The counterpart the omit check owes its own list, and the discipline every other
 * ledger in this repo already has (`lib/fixtures_gate.ts` FAILS on a sanction /
 * known-gap that matched nothing; the injection ratchets refuse a narrowed run).
 * Without it an entry rots dormant after the tool it excuses is fixed, or after an
 * upstream path rename orphans it.
 *
 * What it does NOT catch, and can't: a broad `path` fragment that still matches
 * its ORIGINAL failure stays used, and goes on silently absorbing whatever new
 * failure arrives beneath it. Staleness only ever finds an entry matching
 * nothing; keeping each `path` narrow enough to name one file is still the
 * author's job. The one form of over-breadth that IS caught is the one that
 * reaches another entry's failure — the caller's disjointness check
 * (`perf_omit_matches`) fails on a failure two entries both claim, which is also
 * what keeps a shadowed entry from being reported here as stale.
 *
 * `graded_keys` is the run's REACHABILITY answer — the tracking keys whose tasks
 * both existed and were graded. An entry naming a task no key matches was never
 * asked, so calling it stale would accuse the ledger of a machine's shortfall:
 * every alternative impl is optional (`init_optional`), and one that fails to
 * load registers no task at all, so its files never fail and its entry never
 * fires. Coverage-only keys belong out of this set too — they are exempt from the
 * violation pass, so an entry scoped to one could never be marked used.
 *
 * ⚠️ Even so, only sound over a FULL perf run: a corpus filter (`BENCH_LIMIT` /
 * `BENCH_FILTER`), or a missing corpus repo under `BENCH_ALLOW_MISSING`, can
 * withhold the very files these entries are about while the task itself runs
 * fine — reachability at the task level can't see that. The caller gates on it.
 */
export function stale_perf_omits(
	omits: readonly PerfOmit[],
	used: ReadonlySet<PerfOmit>,
	graded_keys: Iterable<string>
): PerfOmit[] {
	const keys = [...graded_keys];
	return omits.filter(
		(o) => !used.has(o) && keys.some((key) => o.task === undefined || key.includes(o.task))
	);
}
