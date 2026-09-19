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

/**
 * WHY a tool fails a file, as a closed vocabulary — so "is any omit hiding a tsv
 * failure?" is a grep (`tsv_failure`) rather than a reading of seventeen sentences,
 * and the published report can group the omissions.
 *
 * - `tool_limit` — the tool's own limit on valid input: a missing mode, a bug, an
 *   early error it enforces where the file's dialect makes the construct legal.
 *   Nothing the harness does would change the verdict.
 * - `unsupported_syntax` — the tool does not implement the language construct at
 *   all (biome's experimental HTML path on real Svelte blocks).
 * - `harness_path_threading` — the tool WOULD accept the file under its real name;
 *   the bench hands every impl a synthetic `file.ts`, which withholds the
 *   declaration-file mode the content needs. The harness's doing, tolerated
 *   deliberately (see the first entry's comment).
 * - `harvest_artifact` — the file exists only because a harvest made it, and it
 *   inherits a failure from its source (a `<style>` block's concat).
 * - `tsv_failure` — tsv itself fails the file. NONE today, and the one category a
 *   reader should grep for: every other one tolerates a rival's gap.
 */
export type PerfOmitCategory =
	| 'tool_limit'
	| 'unsupported_syntax'
	| 'harness_path_threading'
	| 'harvest_artifact'
	| 'tsv_failure';

/**
 * HOW the tool's refusal arrives — which is what decides whether a wrapper can
 * see it at all (`lib/reject_probe.ts`).
 *
 * - `throw` — the call throws. Visible to any wrapper.
 * - `error_recovered` — a PARSER returns a tree beside error diagnostics, and the
 *   wrapper turns the diagnostics into the rejection (oxc, yuku).
 * - `no_op` — a FORMATTER hands the INPUT back beside diagnostics (biome's
 *   `formatContent`, oxfmt's `format`): read without them, a free file in the
 *   timed sweep and an accept in the coverage.
 */
export type PerfOmitFailure = 'throw' | 'error_recovered' | 'no_op';

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
	/** Why the tool fails it — see `PerfOmitCategory`. */
	category: PerfOmitCategory;
	/** How the refusal arrives — see `PerfOmitFailure`. */
	failure: PerfOmitFailure;
	/** Why this failure is tolerated — keeps the list a reviewed catalogue, never a silent suppressor. */
	reason: string;
}

/**
 * The reviewed perf-corpus failures. Keep this as close to empty as it can be:
 * the perf corpus is real-world code every in-scope tool should handle. Add an
 * entry only for a deliberately-tolerated failure, each with a reason (see the
 * module doc).
 *
 * Most entries date from admitting `.d.ts` files to the corpus (which tsv and
 * prettier fully handle) and tolerate third-party limitations on
 * declaration-file-only syntax; the biome entries are its own parser's limits on
 * real Svelte and declaration files, surfaced once `lib/biome.ts` began reading
 * the diagnostics `formatContent` returns beside an unchanged input:
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
		category: 'tool_limit',
		failure: 'throw',
		reason: 'acorn-typescript cannot parse ambient const declarations (no .d.ts mode)'
	},
	{
		task: 'parse/typescript/oxc',
		path: 'kit/packages/kit/src/runtime/app/env',
		category: 'harness_path_threading',
		failure: 'error_recovered',
		reason:
			'oxc (native + wasm) rejects ambient consts under the synthetic file.ts name (no path threading in the bench)'
	},
	{
		task: 'format/typescript/oxfmt',
		path: 'kit/packages/kit/src/runtime/app/env',
		category: 'harness_path_threading',
		failure: 'no_op',
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
		category: 'harness_path_threading',
		failure: 'error_recovered',
		reason:
			'yuku (native + wasm) rejects ambient consts under the pinned `lang: ts` (its `dts` mode needs path threading the bench does not do)'
	},
	// acorn-typescript enforces the `arguments`-in-class-field-initializer early
	// error; tsv (permissive / defer-diagnostics policy) and prettier accept it.
	{
		task: 'parse/typescript/canonical',
		path: 'svelte/packages/svelte/src/ambient.d.ts',
		category: 'tool_limit',
		failure: 'throw',
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
		category: 'tool_limit',
		failure: 'throw',
		reason: 'swc rejects ambient const declarations even with its own `dts` mode enabled'
	},
	{
		task: 'parse/typescript/swc',
		path: 'svelte/packages/svelte/src/ambient.d.ts',
		category: 'tool_limit',
		failure: 'throw',
		reason:
			'swc enforces the strict-mode eval/arguments binding early error tsv defers (`export const arguments: never`)'
	},
	// biome's `formatContent` reports these as syntax errors and hands the input
	// back unformatted (see `biome_fatal_diagnostics`). The `.d.ts` pair is the same
	// ambient-const shape as above, under the same synthetic `file.ts` name;
	// `motion/public.d.ts` is a class method signature without a body, again
	// declaration-file-only syntax. The svelte ones are biome's experimental HTML
	// path rejecting real Svelte: a multi-statement template expression, a snippet
	// block, `class` parameters, and a `>` selector in a `<style>` block — which
	// its CSS parser rejects again in the harvested copy of that block.
	{
		task: 'format/typescript/biome',
		path: 'kit/packages/kit/src/runtime/app/env',
		category: 'harness_path_threading',
		failure: 'no_op',
		reason:
			'biome rejects ambient consts under the synthetic file.ts name (no path threading in the bench)'
	},
	{
		task: 'format/typescript/biome',
		path: 'svelte/packages/svelte/src/motion/public.d.ts',
		category: 'harness_path_threading',
		failure: 'no_op',
		reason: 'biome rejects a bodiless class method signature under the synthetic file.ts name'
	},
	{
		task: 'format/svelte/biome',
		path: 'zzz/src/lib/Picker.svelte',
		category: 'unsupported_syntax',
		failure: 'no_op',
		reason: 'biome: template expressions can only contain a single expression'
	},
	{
		task: 'format/svelte/biome',
		path: 'zzz/src/lib/PickerDialog.svelte',
		category: 'unsupported_syntax',
		failure: 'no_op',
		reason: 'biome: template expressions can only contain a single expression'
	},
	{
		task: 'format/svelte/biome',
		path: 'zzz/src/lib/SortableList.svelte',
		category: 'unsupported_syntax',
		failure: 'no_op',
		reason: 'biome: template expressions can only contain a single expression'
	},
	{
		task: 'format/svelte/biome',
		path: 'fuz_code/src/routes/docs/benchmark/+page.svelte',
		category: 'unsupported_syntax',
		failure: 'no_op',
		reason: 'biome: expected a closing block (a snippet block its HTML path does not parse)'
	},
	{
		task: 'format/svelte/biome',
		path: 'fuz_css/src/routes/docs/classes/+page.svelte',
		category: 'unsupported_syntax',
		failure: 'no_op',
		reason:
			"biome: expected class parameters but found '<' (a class body its HTML path does not parse)"
	},
	{
		task: 'format/svelte/biome',
		path: 'cosmicplayground/src/routes/StarshipMenu.svelte',
		category: 'unsupported_syntax',
		failure: 'no_op',
		reason: "biome: expected a selector but found '>' in a <style> block"
	},
	// the same `>` selector again, in the harvested per-collection `<style>`
	// concatenation that carries StarshipMenu's block (`svelte_styles_harvest.ts`)
	{
		task: 'format/css/biome',
		path: '.cache/svelte_styles/cosmicplayground.css',
		category: 'harvest_artifact',
		failure: 'no_op',
		reason: "biome: expected a selector but found '>' (the harvested StarshipMenu.svelte styles)"
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

/** One timed row's share of a group's omissions — see `GroupOmissions`. */
export interface ToolOmissions {
	/** The row's display name, the identity every other report field joins on. */
	name: string;
	/** Files this row failed in pre-flight. */
	files: number;
	/** Their UTF-8 size. */
	bytes: number;
	/**
	 * Those files by `PerfOmitCategory`. A failure no `PERF_OMITS` entry claims
	 * counts under `unlisted` — unreachable on a run `enforce_perf_coverage` passed,
	 * and named rather than dropped so the counts always sum to `files`.
	 */
	categories: Record<string, number>;
}

/**
 * What one group's intersection LEFT OUT, and at whose hand.
 *
 * A file any timed row fails leaves EVERY row's timed set (the group is timed on
 * its all-rows intersection), so an omit written against one tool moves every
 * published number in the group — and a file count understates it badly: one
 * harvested stylesheet biome rejects is ~11% of the `format/css` group's bytes.
 * Hence bytes beside files, and the group's totals beside both.
 *
 * `omitted_*` is the UNION over rows, not the sum: two rows failing one file omit
 * it once. `by_tool` is per row, so its counts can sum past it.
 */
export interface GroupOmissions {
	/** `operation/language`. */
	group: string;
	files_total: number;
	bytes_total: number;
	omitted_files: number;
	omitted_bytes: number;
	/** Rows that failed at least one file, in the order given. */
	by_tool: ToolOmissions[];
}

/**
 * Summarize one group's omissions from its pre-flight failures.
 *
 * Pure, so the arithmetic the published disclosure rests on is testable without a
 * corpus. The caller passes TIMED rows only: a coverage-only row never narrows the
 * intersection, so its failures omit nothing.
 *
 * @param group - the group's `operation/language` name
 * @param files - every file the group loaded
 * @param rows - each timed row, with the paths it failed in pre-flight
 * @param omits - the ledger to categorize against
 */
export function summarize_group_omissions(
	group: string,
	files: ReadonlyArray<{ path: string; bytes: number }>,
	rows: ReadonlyArray<{ name: string; tracking_key: string; failed: Iterable<string> }>,
	omits: readonly PerfOmit[]
): GroupOmissions {
	const bytes_by_path = new Map(files.map((f) => [f.path, f.bytes]));
	const omitted = new Set<string>();
	const by_tool: ToolOmissions[] = [];
	for (const row of rows) {
		const tool: ToolOmissions = { name: row.name, files: 0, bytes: 0, categories: {} };
		for (const path of row.failed) {
			// A failure on a file outside `files` is not this group's to report.
			const bytes = bytes_by_path.get(path);
			if (bytes === undefined) continue;
			omitted.add(path);
			tool.files += 1;
			tool.bytes += bytes;
			const matches = perf_omit_matches(omits, row.tracking_key, path);
			const category = matches.length > 0 ? matches[0].category : 'unlisted';
			tool.categories[category] = (tool.categories[category] ?? 0) + 1;
		}
		if (tool.files > 0) by_tool.push(tool);
	}
	let omitted_bytes = 0;
	for (const path of omitted) omitted_bytes += bytes_by_path.get(path)!;
	return {
		group,
		files_total: files.length,
		bytes_total: files.reduce((sum, f) => sum + f.bytes, 0),
		omitted_files: omitted.size,
		omitted_bytes,
		by_tool
	};
}
