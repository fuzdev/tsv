/**
 * The `loc` cross-grade: tsv's two implementations of one `loc` definition, graded against
 * each other over every parseable fixture document.
 *
 * `loc` is defined once (every object with numeric `start`/`end` gets the line and UTF-16
 * column of those offsets, under one line-terminator rule per document — see
 * `crates/tsv_wasm/npm/locations.js`'s module doc) and implemented twice: by the Rust
 * writers, which emit it on the loc-bearing wire, and by the shipped `locations.js`, which
 * reconstructs it in JS from the span-only wire plus the source. Neither carries a
 * hand-written expectation; each is the other's drift check. The fixtures pin the span-only
 * wire, so this is the only `npm run check` leg that grades the JS reconstruction
 * (`tests/loc_definition.rs` grades the Rust loc wire against an independent reference). (Its outside
 * reference — acorn's and Svelte's own `loc` — is graded at conformance cadence by
 * `corpus:compare:parse`, whose loc arm runs this same reconstruction over the corpus.)
 *
 * The Rust half is one `tsv_debug loc_wires` process streaming both wires of every
 * document as NDJSON — no per-file spawn, no sidecar, no `node_modules`. The documents are
 * each fixture's input and its format variants, which need no oracle (`loc_wires`' module
 * doc): one the fixture tree requires tsv to parse fails the run if it does not, and a
 * prettier-side form tsv rejects (prettier's output is not always valid source) arrives as
 * a `rejected` record. Those rejects are a LEDGER, `check_loc_rejects.txt` beside this
 * script: the exact documents prettier emits that the validity oracle for the syntax they
 * break rejects (Svelte's parser, tsc, the CSS spec — some of them ones the canonical
 * acorn / Svelte parser accepts, since tsc is the TypeScript oracle, CLAUDE.md
 * §Strictness), graded both ways so a tsv regression that starts rejecting a prettier
 * output it parses today fails rather than counting one more reject. This half
 * reconstructs each span-only wire with the shipped module and deep-equals the result
 * against the loc wire, key order ignored (the Rust writer places `loc` after `end`, the
 * reconstruction appends it last; the contract leaves order unspecified).
 *
 * Usage: `npm run check:loc` (builds `target/corpus/tsv_debug` first), or directly,
 * `node scripts/roots/check_loc.ts [root]`, the root a path in the checkout.
 *
 * Its reach: the checkout read-only (the ledger) and the checkout's `target/corpus/tsv_debug`,
 * run in it (`CheckLocDeps`, built by `scripts/roots/check_loc.ts`).
 */

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram, SpawnResult } from '@fuzdev/gro/spawn.ts';

import { loc_definition_violation } from '../benches/js/lib/loc_cross_grade.ts';
import type { Language } from '../benches/js/lib/types.ts';
import { lines_of } from '../benches/js/lib/text_lines.ts';
import { signal_exit_code } from './exit_code.ts';

/** The `tsv_debug` binary of the shared `--profile corpus` build world. */
export const CHECK_LOC_TSV_DEBUG = 'target/corpus/tsv_debug';

const DEFAULT_ROOT = 'tests/fixtures';
/** The prettier-side documents tsv rejects, relative to `DEFAULT_ROOT` — the ledger. */
const REJECTS_LEDGER = 'scripts/check_loc_rejects.txt';

/** What `run_check_loc` reaches. */
export interface CheckLocDeps {
	/** The checkout, read for the ledger and `tsv_debug`'s working directory. */
	checkout: FsReadDir;
	/** The checkout's `target/corpus/tsv_debug` (`CHECK_LOC_TSV_DEBUG`). */
	tsv_debug: SpawnProgram;
}
/** Mismatching documents listed in full before the rest are only counted. */
const MAX_REPORTED = 20;

/** One graded document's two wires. */
interface WireRecord {
	path: string;
	language: Language;
	source: string;
	loc: unknown;
	span: unknown;
}

/** A prettier-side document tsv rejects — no claim says it parses, so the ledger grades it. */
interface RejectedRecord {
	path: string;
	rejected: string;
}

/** The ledger's paths: one per line, `#` to the line's end a comment. */
async function read_rejects_ledger(checkout: FsReadDir): Promise<Set<string>> {
	const text = await checkout.read_text(REJECTS_LEDGER);
	const paths = new Set<string>();
	for (const line of text.split('\n')) {
		const path = line.replace(/#.*/, '').trim();
		if (path !== '') paths.add(path);
	}
	return paths;
}

/** A `loc_wires` path relative to the fixture tree, as the ledger spells it. */
function fixture_relative(path: string): string {
	const marker = `${DEFAULT_ROOT}/`;
	const at = path.lastIndexOf(marker);
	return at === -1 ? path.replace(/^\.\//, '') : path.slice(at + marker.length);
}

/**
 * The root as a checkout-relative path with no `./` prefix, doubled `/`, or trailing `/`, so
 * the paths `loc_wires` emits under it line up with the ledger's.
 */
export function normalize_root(root: string, checkout_path: string): string {
	const prefix = `${checkout_path}/`;
	let path = root.startsWith(prefix) ? root.slice(prefix.length) : root;
	path = path.replace(/\/{2,}/g, '/');
	while (path.startsWith('./')) path = path.slice(2);
	return path.replace(/\/+$/, '') || '.';
}

/** Whether a ledger entry lies under `root` — the entries a run over `root` can grade stale. */
function ledger_entry_under(path: string, root: string): boolean {
	return root === '.' || `${DEFAULT_ROOT}/${path}`.startsWith(`${root}/`);
}

/**
 * The code `loc_wires` ended with, as the failure line names it: its exit code, `128 + N` for
 * signal `N`.
 */
export function to_ended_code(status: SpawnResult): number {
	if (status.kind === 'exited') return status.code;
	if (status.kind === 'signaled') return signal_exit_code(status.signal);
	return status.signal === null ? (status.code ?? 1) : signal_exit_code(status.signal);
}

/**
 * The cross-grade over `args[0]`, the fixture tree by default.
 * @returns the exit code: `1` on any failure, else `0`
 */
export async function run_check_loc(
	deps: CheckLocDeps,
	args: ReadonlyArray<string>
): Promise<number> {
	const root = normalize_root(args[0] ?? DEFAULT_ROOT, deps.checkout.path);
	const ledger = await read_rejects_ledger(deps.checkout);
	const started = performance.now();
	const child = await deps.tsv_debug.start(['loc_wires', root], {
		cwd: deps.checkout,
		stdio: ['inherit', 'pipe', 'inherit']
	});

	const graded: Record<Language, number> = { svelte: 0, typescript: 0, css: 0 };
	const mismatches: string[] = [];
	const rejected = new Set<string>();
	for await (const line of lines_of(child.stdout!)) {
		if (line === '') continue;
		const parsed = JSON.parse(line) as WireRecord | RejectedRecord;
		if ('rejected' in parsed) {
			rejected.add(fixture_relative(parsed.path));
			continue;
		}
		const record = parsed;
		const difference = loc_definition_violation(
			record.loc,
			record.span,
			record.source,
			record.language
		);
		if (difference !== null) {
			mismatches.push(`${record.path} (${record.language}): wire vs reconstruct at ${difference}`);
		}
		graded[record.language]++;
	}
	const status = await child.closed;
	const seconds = ((performance.now() - started) / 1000).toFixed(1);
	const total = graded.svelte + graded.typescript + graded.css;
	console.log(
		`loc cross-grade: ${total} documents (${graded.svelte} svelte, ${graded.typescript} typescript, ${graded.css} css) in ${seconds}s` +
			` — ${rejected.size} prettier-side form(s) tsv rejects, graded against check_loc_rejects.txt`
	);

	const failures: string[] = [];
	const code = to_ended_code(status);
	if (code !== 0) failures.push(`\`${CHECK_LOC_TSV_DEBUG} loc_wires\` exited ${code}`);
	// Every language reached the grade: a walk that silently stopped producing one would
	// leave it ungraded while the run reads green. Only the whole tree holds all three; a
	// narrowed root (one language's subtree) is held to grading something.
	if (root === DEFAULT_ROOT) {
		for (const [language, count] of Object.entries(graded)) {
			if (count === 0) failures.push(`no ${language} document was graded`);
		}
	} else if (total === 0) {
		failures.push(`no document under ${root} was graded`);
	}
	const new_rejects = [...rejected].filter((path) => !ledger.has(path)).sort();
	if (new_rejects.length > 0) {
		for (const path of new_rejects) console.log(`  new reject: ${path}`);
		failures.push(
			`${new_rejects.length} prettier-side document(s) tsv rejects that check_loc_rejects.txt does not list — a tsv regression, or a new fixture whose prettier output is invalid (list it)`
		);
	}
	// a narrowed root walks only the entries under it, so only those can read stale
	const stale = [...ledger]
		.filter((path) => ledger_entry_under(path, root) && !rejected.has(path))
		.sort();
	if (stale.length > 0) {
		for (const path of stale) console.log(`  stale ledger entry: ${path}`);
		failures.push(
			`${stale.length} check_loc_rejects.txt entries tsv now parses or the tree no longer holds — delete them`
		);
	}
	if (mismatches.length > 0) {
		for (const m of mismatches.slice(0, MAX_REPORTED)) console.log(`  ${m}`);
		if (mismatches.length > MAX_REPORTED) {
			console.log(`  ... and ${mismatches.length - MAX_REPORTED} more`);
		}
		failures.push(
			`${mismatches.length} document(s) whose loc wire differs from the reconstruction of their span-only wire`
		);
	}
	if (failures.length > 0) {
		console.log(`FAIL: ${failures.join('; ')}`);
		return 1;
	}
	console.log(
		'PASS: the loc wire equals the reconstruction of the span-only wire on every document'
	);
	return 0;
}
