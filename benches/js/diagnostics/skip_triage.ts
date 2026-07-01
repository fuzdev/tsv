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
 *                             path is in `SANCTIONED` below.
 *   - `canonical_fails_tsv_ok` — tsv accepts input the canonical parser rejects:
 *                             an over-acceptance, usually a deferred early-error
 *                             (tsv's documented posture). Reported, does not gate.
 *
 * The gate fails (exit 1) only on an **un-sanctioned over-rejection** — a tsv
 * parse gap on valid input that isn't already catalogued. Point it at a corpus
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
import type { Language } from '../lib/types.ts';

/**
 * Over-rejections (tsv rejects, canonical accepts) that are deliberate, not
 * gaps — matched as a substring of the file path. Each needs a reason so the
 * list stays a reviewed catalogue, never a silent bug suppressor. Adding one is
 * a claim that tsv is *correctly* stricter (or that the input is invalid Svelte
 * the canonical parser is merely lenient about); a genuine gap gets fixed, not
 * listed. These cover Svelte's own `tests/` fixtures; real source needs none.
 */
const SANCTIONED: { pattern: string; reason: string }[] = [
	// CSS grammar-strictness — Svelte's parser is lenient where tsv follows the
	// CSS grammar. See docs/conformance_svelte.md §CSS Parser Scope & Error Model.
	{
		pattern: 'css/samples/comment-html/',
		reason: 'HTML comment (`<!-- -->`) in a CSS selector — svelte lenient, tsv grammar-stricter',
	},
	{
		pattern: 'css/samples/supports-import/',
		reason: '`@import` inside `@supports` prelude — svelte lenient, tsv grammar-stricter',
	},
	{
		pattern: 'validator/samples/css-invalid-combinator-selector',
		reason: 'invalid leading combinator (`>`/`+`) — svelte parser accepts, its validator rejects',
	},
	// Invalid Svelte markup — Svelte's PARSER accepts, its VALIDATOR rejects; the
	// input is invalid either way, tsv just rejects one stage earlier.
	{
		pattern: 'validator/samples/attribute-invalid-name',
		reason: 'invalid attribute-name character — svelte parser lenient, validator rejects',
	},
	{
		pattern: 'validator/samples/if-block-whitespace',
		reason: 'whitespace after `{#` (`{ #if}`) — svelte parser lenient, validator rejects',
	},
];

function sanction_for(path: string): string | null {
	return SANCTIONED.find((s) => path.includes(s.pattern))?.reason ?? null;
}

const corpus_path = Deno.args.find((a) => !a.startsWith('-'));

const [files, impls] = await Promise.all([
	(corpus_path ? new DirectoryLoader(corpus_path) : new DevReposLoader()).load((m) =>
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
			const reason = sanction_for(f.path);
			if (reason) {
				buckets.sanctioned_over_rejection.push({ path: f.path, error: tsv_err, reason });
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
		`\nFAIL: ${unexpected_total} un-sanctioned over-rejection(s) — tsv rejects input the ` +
			`canonical parser accepts. Fix the parser, or (if tsv is correctly stricter) add a ` +
			`reasoned entry to SANCTIONED in this file.`,
	);
	Deno.exit(1);
}
console.error('\nOK: no un-sanctioned parse over-rejections (parity holds).');
