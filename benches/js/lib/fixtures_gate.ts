/**
 * Shared engine for the per-language parse-conformance fixtures gates
 * (`diagnostics/svelte_fixtures_compare.ts`, `diagnostics/ts_fixtures_compare.ts`)
 * — the drop-in-parser analogs of test262 (JS) that run tsv against a canonical
 * parser's OWN adversarial test suite.
 *
 * Every such gate does the identical procedure — discover the canonical inputs
 * under a root, parse each with tsv + the canonical parser, bucket by verdict
 * asymmetry (parity / over-acceptance / over-rejection → sanctioned·known-gap·
 * unexpected), deep-diff the both-accept ASTs, and emit a stderr summary + JSON
 * report, exiting 1 only on an `unexpected` over-rejection. Only the *config*
 * differs (root, input basenames, prune rule, language, sanction/known-gap
 * lists). Keeping the procedure here means a fix to the gate/report logic applies
 * to every language and the gates can't drift; each script stays a docstring +
 * config that explains its own oracle rationale.
 *
 * The AST-shape half reuses the SHARED `corpus_compare_parse.ts` diff engine
 * (`diff_asts` + `DOCUMENTED_MATCHERS`, `import.meta.main`-guarded so importing it
 * is side-effect-free), so a divergence cataloged there also shrinks the
 * `corpus:compare:parse` count. Verdict parity GATES; AST-shape is report-only.
 */

import { readdir, readFile } from 'node:fs/promises';
import { join } from 'node:path';

import { init_compare_implementations } from './compare_cli.ts';
import { type Sanction, sanction_for } from './parse_sanctions.ts';
import type { Language } from './types.ts';
import { bigint_replacer, type DiffEntry, diff_asts, type MatchContext } from '../corpus_compare_parse.ts';

/**
 * An over-rejection where tsv is WRONG — a genuine drop-in gap, tracked so the
 * gate is green at baseline and only a NEW untracked over-rejection fails it.
 * Matched as a path substring. This set must only SHRINK: delete an entry once
 * its gap is fixed (the input then parses → parity).
 */
export interface KnownGap {
	pattern: string;
	category: string;
	reason: string;
}

export interface FixturesGateConfig {
	/** Display title, e.g. `TypeScript-fixtures`. */
	title: string;
	/** The `Language` both parsers are invoked with. */
	language: Language;
	/** Suite root when no positional arg is given. */
	default_root: string;
	/** Canonical parse-INPUT basenames (excludes harness/output files). */
	input_basenames: Set<string>;
	/** Noun for the scanned-count line, e.g. `.ts inputs`. */
	input_noun: string;
	/** Directory names pruned wholesale during discovery. */
	prune_dir: (name: string) => boolean;
	/** Over-rejections tsv keeps deliberately (from `lib/parse_sanctions.ts`). */
	sanctioned: Sanction[];
	/** Parenthetical for the sanctioned report line, e.g. `tsv correctly stricter`. */
	sanctioned_note: string;
	/** Over-rejections tsv is wrong about — tracked, must only shrink. */
	known_gaps: KnownGap[];
	/** Canonical parser name for the FAIL message, e.g. `acorn-typescript`. */
	oracle_name: string;
}

interface OverRejection {
	path: string;
	error: string;
}

/** AST-diff groups keyed by signature, split documented vs undocumented. */
interface AstGroup {
	signature: string;
	documented: string | null;
	files: Set<string>;
	count: number;
	sample: { path: string; entry: DiffEntry } | null;
}

async function* discover(
	root: string,
	config: FixturesGateConfig,
): AsyncGenerator<{ path: string; content: string }> {
	let entries;
	try {
		entries = await readdir(root, { withFileTypes: true });
	} catch (e) {
		console.error(`Cannot read ${root}: ${e instanceof Error ? e.message : e}`);
		return;
	}
	for (const entry of entries) {
		const full = join(root, entry.name);
		if (entry.isDirectory()) {
			if (!config.prune_dir(entry.name)) yield* discover(full, config);
		} else if (config.input_basenames.has(entry.name)) {
			yield { path: full, content: await readFile(full, 'utf8') };
		}
	}
}

function first_line(e: unknown): string {
	return String(e instanceof Error ? e.message : e).split('\n')[0];
}

/**
 * Run a per-language fixtures parse-conformance gate. Reads `Deno.args`
 * (`--json`, `--verbose`/`-v`, optional positional root override), scans the
 * suite, prints a summary to stderr (+ JSON to stdout under `--json`), and
 * exits 1 on any `unexpected` over-rejection.
 */
export async function run_fixtures_gate(config: FixturesGateConfig): Promise<void> {
	const flags = new Set(Deno.args.filter((a) => a.startsWith('-')));
	const json_mode = flags.has('--json');
	const verbose = flags.has('--verbose') || flags.has('-v');
	const root = Deno.args.find((a) => !a.startsWith('-')) ?? config.default_root;

	const { canonical, native } = await init_compare_implementations();

	const buckets = {
		parity: 0,
		both_accept: 0,
		over_acceptance: [] as OverRejection[],
		sanctioned: [] as (OverRejection & { reason: string })[],
		known_gap: [] as (OverRejection & { category: string; reason: string })[],
		unexpected: [] as OverRejection[],
	};

	const ast_groups = new Map<string, AstGroup>();
	let ast_clean = 0;

	function record_ast(path: string, diffs: DiffEntry[]): void {
		if (diffs.length === 0) {
			ast_clean++;
			return;
		}
		for (const entry of diffs) {
			const key = `${entry.signature}\0${entry.documented ?? ''}`;
			let g = ast_groups.get(key);
			if (!g) {
				g = { signature: entry.signature, documented: entry.documented, files: new Set(), count: 0, sample: null };
				ast_groups.set(key, g);
			}
			g.files.add(path);
			g.count++;
			if (!g.sample) g.sample = { path, entry };
		}
	}

	let scanned = 0;
	for await (const file of discover(root, config)) {
		scanned++;

		let tsv_ast: unknown;
		let tsv_err: string | null = null;
		try {
			tsv_ast = native.parse(file.content, config.language);
		} catch (e) {
			tsv_err = first_line(e);
		}

		let canon_ast: unknown;
		let canon_err: string | null = null;
		try {
			canon_ast = canonical.parse(file.content, config.language);
		} catch (e) {
			canon_err = first_line(e);
		}

		if (tsv_err && canon_err) {
			buckets.parity++;
		} else if (tsv_err && !canon_err) {
			// Over-rejection — classify sanctioned / known-gap / unexpected.
			const reason = sanction_for(config.sanctioned, file.path);
			const gap = config.known_gaps.find((g) => file.path.includes(g.pattern));
			if (reason) {
				buckets.sanctioned.push({ path: file.path, error: tsv_err, reason });
			} else if (gap) {
				buckets.known_gap.push({ path: file.path, error: tsv_err, category: gap.category, reason: gap.reason });
			} else {
				buckets.unexpected.push({ path: file.path, error: tsv_err });
			}
		} else if (!tsv_err && canon_err) {
			buckets.over_acceptance.push({ path: file.path, error: canon_err });
		} else {
			// Both accept — deep-diff the ASTs (canonical serialized like the sidecar).
			buckets.both_accept++;
			const canonical_root = JSON.parse(JSON.stringify(canon_ast, bigint_replacer));
			const ctx: MatchContext = { source: file.content, canonical_root };
			const { diffs } = diff_asts(tsv_ast, canonical_root, ctx);
			record_ast(file.path, diffs);
		}
	}

	canonical.dispose();
	native.dispose();

	// --- Report -----------------------------------------------------------------

	const undocumented_groups = [...ast_groups.values()].filter((g) => g.documented === null);
	const documented_groups = [...ast_groups.values()].filter((g) => g.documented !== null);

	const gap_by_category = new Map<string, number>();
	for (const g of buckets.known_gap) {
		gap_by_category.set(g.category, (gap_by_category.get(g.category) ?? 0) + 1);
	}

	console.error(`\n${config.title} parse-conformance gate — root: ${root}`);
	console.error(`  scanned: ${scanned} ${config.input_noun}\n`);
	console.error(`  VERDICT`);
	console.error(`    parity (both reject):     ${buckets.parity}`);
	console.error(`    both accept:              ${buckets.both_accept}`);
	console.error(`    over-acceptance:          ${buckets.over_acceptance.length}  (deferred early-errors; not gated)`);
	console.error(`    over-rejection sanctioned:${buckets.sanctioned.length}  (${config.sanctioned_note})`);
	console.error(
		`    over-rejection known-gap: ${buckets.known_gap.length}  (tracked; ${
			[...gap_by_category.entries()].map(([c, n]) => `${c}=${n}`).join(', ') || 'none'
		})`,
	);
	console.error(`    over-rejection UNEXPECTED: ${buckets.unexpected.length}  (new gap — GATES)`);
	console.error(`\n  AST-SHAPE (of the ${buckets.both_accept} both-accept)`);
	console.error(`    clean:                    ${ast_clean}`);
	console.error(`    documented diff groups:   ${documented_groups.length}`);
	console.error(`    undocumented diff groups: ${undocumented_groups.length}  (report-only — triage surface)`);

	for (const e of buckets.unexpected) {
		console.error(`\n    ✗ UNEXPECTED over-rejection: ${e.path}\n        ${e.error}`);
	}
	// AST-shape groups are a report-only triage surface — full detail only with -v
	// (or the --json report), so a green verdict run isn't buried under the groups.
	if (verbose) {
		for (const g of undocumented_groups) {
			console.error(
				`      · undocumented AST group: ${g.signature}  (${g.count} in ${g.files.size} file(s))` +
					(g.sample ? `  e.g. ${g.sample.path}` : ''),
			);
		}
		for (const g of buckets.known_gap) console.error(`      · known-gap [${g.category}] ${g.path}`);
		for (const e of buckets.over_acceptance) console.error(`      · over-acceptance ${e.path}`);
	}

	const report = {
		root,
		scanned,
		verdict: {
			parity: buckets.parity,
			both_accept: buckets.both_accept,
			over_acceptance: buckets.over_acceptance,
			sanctioned: buckets.sanctioned,
			known_gap: buckets.known_gap,
			known_gap_by_category: Object.fromEntries(gap_by_category),
			unexpected: buckets.unexpected,
		},
		ast: {
			both_accept: buckets.both_accept,
			clean: ast_clean,
			documented_groups: documented_groups.map((g) => ({
				signature: g.signature,
				matcher: g.documented,
				count: g.count,
				files: g.files.size,
			})),
			undocumented_groups: undocumented_groups.map((g) => ({
				signature: g.signature,
				count: g.count,
				files: [...g.files].slice(0, 10),
				sample: g.sample ? { path: g.sample.path, entry: g.sample.entry } : null,
			})),
		},
	};

	if (json_mode) {
		Deno.stdout.writeSync(new TextEncoder().encode(JSON.stringify(report, null, '\t') + '\n'));
	}

	if (undocumented_groups.length > 0) {
		console.error(
			`\nNOTE: ${undocumented_groups.length} undocumented AST-shape group(s) to triage (report-only, not ` +
				`gating) — catalog each into corpus_compare_parse.ts DOCUMENTED_MATCHERS (shared; also shrinks ` +
				`the corpus:compare:parse count) or fix as a writer/parser bug. Detail: -v or --json.`,
		);
	}
	// Only the verdict half enforces: a NEW over-rejection in neither the sanction
	// list nor KNOWN_GAPS is a genuine drop-in regression.
	if (buckets.unexpected.length > 0) {
		console.error(
			`\nFAIL: ${buckets.unexpected.length} unexpected over-rejection(s) — tsv rejects input ${config.oracle_name} ` +
				`accepts, and it's neither sanctioned nor a tracked gap. Fix the parser, or — if tsv is correctly ` +
				`stricter — add a reasoned sanction entry; if it's a known gap, add a KNOWN_GAPS entry.`,
		);
		Deno.exit(1);
	}
	console.error(`\nOK: verdict parity holds (no unexpected over-rejections).`);
}
