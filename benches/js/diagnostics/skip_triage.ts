/**
 * Parse-parity gate: which corpus files does tsv fail to parse, and does the
 * canonical parser (svelte/compiler / acorn-typescript / parseCss) handle them?
 *
 * The point is to **assert parity on failure, not count raw errors**. A parse
 * rejection is only interesting when it is *asymmetric* — the two parsers
 * disagree. So the three buckets are:
 *
 *   - `both_fail`            — PARITY. Both reject (an intentional-error fixture,
 *                             invalid input). The healthy state; never gates.
 *   - `tsv_fails_canonical_ok` — tsv rejects input the canonical parser accepts:
 *                             an **over-rejection**. A real drop-in gap unless the
 *                             path is catalogued below — in `SANCTIONED` (a
 *                             deliberate divergence: deprecated syntax or a permanent
 *                             non-goal) or `KNOWN_GAPS` (tsv wrong, a tracked gap to
 *                             fix — skip_triage's analog of the dedicated gates'
 *                             KNOWN_GAPS, since it has no per-language gate).
 *   - `canonical_fails_tsv_ok` — tsv accepts input the canonical parser rejects:
 *                             an over-acceptance, usually a deferred early-error
 *                             (tsv's documented posture). Reported, does not gate.
 *
 * The gate fails (exit 1) only on an **untracked over-rejection** — a tsv
 * parse gap on valid input in neither `SANCTIONED` nor `KNOWN_GAPS`. Point it at a corpus
 * with a directory argument (defaults to the ~/dev repos):
 *
 *   # real source — expect green (valid code parses in both)
 *   deno run --allow-ffi --allow-read --allow-env --allow-net --allow-sys \
 *     benches/js/diagnostics/skip_triage.ts
 *   # svelte's own adversarial fixture suite — parity + the residual gap list
 *   deno run --allow-ffi --allow-read --allow-env --allow-net --allow-sys \
 *     benches/js/diagnostics/skip_triage.ts ../svelte/packages/svelte/tests
 *
 * Full JSON to stdout, summary to stderr (`2>/dev/null` for a clean stream).
 */

import { DevReposLoader, DirectoryLoader, group_by_language } from '../lib/corpus.ts';
import { init_implementations } from '../lib/implementations.ts';
import { type KnownGap, type Sanction, sanction_for, SVELTE_FIXTURE_SANCTIONS } from '../lib/parse_sanctions.ts';
import type { Language } from '../lib/types.ts';

/**
 * Over-rejections (tsv rejects, canonical accepts) that are deliberate, not
 * gaps — matched as a substring of the file path. Each needs a reason so the
 * list stays a reviewed catalogue, never a silent bug suppressor. Adding one is
 * a claim that tsv is *correctly* stricter (or that the input is invalid the
 * canonical parser is merely lenient about); a genuine gap gets fixed, not
 * listed. The Svelte-fixture entries are shared (`lib/parse_sanctions.ts`, also
 * used by the dedicated `svelte_fixtures_compare.ts` gate); the entries below
 * cover the prettier test corpus, which only this general sweep scans.
 */
const SANCTIONED: Sanction[] = [
	...SVELTE_FIXTURE_SANCTIONS,
	// Legacy Stage-3 import-assertions `assert { … }` clause — never merged into
	// ecma262 (the final WithClause grammar is `with`-only) and since removed from
	// engines; acorn-typescript still accepts it. Deliberate: deprecated syntax
	// declined for its successor `with {…}` (which tsv parses). See
	// docs/conformance_svelte.md §TypeScript Corrections.
	{
		pattern: 'tests/format/js/import-assertions/',
		reason: 'legacy `assert {…}` import attributes — abandoned pre-spec form; tsv is `with`-only per ecma262',
	},
	// IE property/selector hacks — proprietary syntax outside the CSS grammar, a
	// PERMANENT non-goal (docs/conformance_svelte.md §CSS Parser Scope). tsv is
	// spec-only; this never becomes valid, so it's a true sanction, not a gap.
	{
		pattern: 'tests/format/css/stylefmt-repo/ie-hacks/',
		reason: 'IE property/selector hacks — proprietary syntax outside the CSS grammar; tsv is spec-only (permanent non-goal)',
	},
	// Not a Svelte component — an HTML conformance file the corpus loader feeds
	// to the Svelte parser; svelte/compiler happens to tolerate its raw `[`.
	{
		pattern: 'tests/format/html/tags/tags.html',
		reason: '.html file, not Svelte — raw template `[` svelte tolerates; out of tsv scope',
	},
];

// Over-rejections where tsv is grammar-correct on INVALID CSS but hard-fails the whole
// file where the CSS spec RECOVERS (drop the bad rule, keep the rest). NOT sanctions —
// tsv will accept the file once error recovery lands (docs/conformance_svelte.md §CSS
// Parser Scope & Error Model). skip_triage has no per-language gate, so its KNOWN_GAPS
// (the dedicated-gate concept) live here; tracked so they don't read as `unexpected`.
const KNOWN_GAPS: KnownGap[] = [
	{
		pattern: 'tests/format/css/attribute/quotes.css',
		category: 'css-error-recovery',
		reason: 'function as attr value `[id=func("foo")]` — invalid per selectors-4 (<string>|<ident>); recovery drops the rule',
	},
	{
		pattern: 'tests/format/css/attribute/sensitive.css',
		category: 'css-error-recovery',
		reason: 'invalid attr case-flag `[type=a x]` — selectors-4 <attr-modifier> is `i`|`s` only; recovery drops the rule',
	},
	{
		pattern: 'tests/format/css/loose/loose.css',
		category: 'css-error-recovery',
		reason: 'whitespace before `(` in `calc (…)` / split calc() — invalid function-token grammar (ident + `(` must be adjacent); parseCss+prettier lenient, recovery drops the rule',
	},
	// A REAL tsv CSS bug (not error recovery): valid CSS both oracles accept, tsv wrongly rejects.
	{
		pattern: 'tests/format/css/inline-url/inline_url.css',
		category: 'css-url-string',
		reason: 'quoted `url(\'…{}…\')` with special chars inside the string — VALID CSS (parseCss + prettier accept); tsv over-rejects with `Expected }` (treats the `{` inside the string as a block open)',
	},
];

const corpus_path = Deno.args.find((a) => !a.startsWith('-'));

const [files, impls] = await Promise.all([
	(corpus_path ? new DirectoryLoader(corpus_path) : new DevReposLoader('gates')).load((m) =>
		console.error(m)
	),
	init_implementations({ logger: (m) => console.error(m) }),
]);
const by_language = group_by_language(files);
if (!impls.native) throw new Error('native FFI not built');

const langs: Language[] = ['svelte', 'typescript', 'css'];

interface Entry {
	path: string;
	error: string;
}
interface Buckets {
	/** Over-rejections with no sanction — the gap list that fails the gate. */
	unexpected_over_rejection: Entry[];
	/** Over-rejections matched by `SANCTIONED` — deliberate, catalogued. */
	sanctioned_over_rejection: (Entry & { reason: string })[];
	/** Over-rejections matched by `KNOWN_GAPS` — tsv wrong but tracked (invalid-CSS pending recovery). */
	known_gap_over_rejection: (Entry & { reason: string })[];
	/** tsv accepts, canonical rejects — over-acceptance (deferred early-errors). */
	over_acceptance: Entry[];
	/** Both reject — parity, the healthy state. */
	parity: { path: string; tsv_error: string; canonical_error: string }[];
}

const report: Record<string, Buckets> = {};

for (const lang of langs) {
	const buckets: Buckets = {
		unexpected_over_rejection: [],
		sanctioned_over_rejection: [],
		known_gap_over_rejection: [],
		over_acceptance: [],
		parity: [],
	};
	for (const f of by_language[lang]) {
		let tsv_err: string | null = null;
		let canon_err: string | null = null;
		try {
			// parse_internal suffices: only throw/no-throw is read, and it skips
			// the full JSON materialization (Rust to_string + FFI copy + JS
			// JSON.parse) — same $lang::parse + error surface in tsv_ffi
			impls.native!.parse_internal(f.content, lang);
		} catch (e) {
			tsv_err = String(e instanceof Error ? e.message : e).split('\n')[0];
		}
		try {
			impls.canonical.parse(f.content, lang);
		} catch (e) {
			canon_err = String(e instanceof Error ? e.message : e).split('\n')[0];
		}
		if (tsv_err && !canon_err) {
			const reason = sanction_for(SANCTIONED, f.path);
			const gap = KNOWN_GAPS.find((g) => f.path.includes(g.pattern));
			if (reason) {
				buckets.sanctioned_over_rejection.push({ path: f.path, error: tsv_err, reason });
			} else if (gap) {
				buckets.known_gap_over_rejection.push({ path: f.path, error: tsv_err, reason: gap.reason });
			} else {
				buckets.unexpected_over_rejection.push({ path: f.path, error: tsv_err });
			}
		} else if (!tsv_err && canon_err) {
			buckets.over_acceptance.push({ path: f.path, error: canon_err });
		} else if (tsv_err && canon_err) {
			buckets.parity.push({ path: f.path, tsv_error: tsv_err, canonical_error: canon_err });
		}
	}
	report[lang] = buckets;
}

// Summary to stderr, full JSON to stdout.
let unexpected_total = 0;
for (const lang of langs) {
	const b = report[lang];
	unexpected_total += b.unexpected_over_rejection.length;
	console.error(
		`\n${lang}: parity(both-reject)=${b.parity.length}  ` +
			`sanctioned-over-rejection=${b.sanctioned_over_rejection.length}  ` +
			`known-gap-over-rejection=${b.known_gap_over_rejection.length}  ` +
			`over-acceptance=${b.over_acceptance.length}  ` +
			`UNEXPECTED-over-rejection=${b.unexpected_over_rejection.length}`,
	);
	for (const e of b.unexpected_over_rejection) {
		console.error(`    ✗ ${e.path}\n        ${e.error}`);
	}
}

console.log(JSON.stringify(report, null, 2));

if (unexpected_total > 0) {
	console.error(
		`\nFAIL: ${unexpected_total} untracked over-rejection(s) — tsv rejects input the ` +
			`canonical parser accepts. Fix the parser, or add a reasoned entry to SANCTIONED ` +
			`(a deliberate divergence — deprecated syntax or a permanent non-goal) / KNOWN_GAPS ` +
			`(tsv wrong, a tracked gap to fix) in this file.`,
	);
	Deno.exit(1);
}
console.error('\nOK: no un-sanctioned parse over-rejections (parity holds).');
