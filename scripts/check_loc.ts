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
 * wire, so this is the only `deno task check` leg that grades the JS reconstruction
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
 * Usage: `deno task check:loc` (builds `target/corpus/tsv_debug` first), or directly
 * `deno run --allow-read --allow-run=target/corpus/tsv_debug scripts/check_loc.ts [root]`.
 */

import { loc_definition_violation } from '../benches/js/lib/loc_cross_grade.ts';
import { TSV_DEBUG_CORPUS as TSV_DEBUG } from '../benches/js/lib/loc_wire_client.ts';
import type { Language } from '../benches/js/lib/types.ts';
import { lines_of } from '../benches/js/lib/text_lines.ts';

const DEFAULT_ROOT = 'tests/fixtures';
/** The prettier-side documents tsv rejects, relative to `DEFAULT_ROOT` — the ledger. */
const REJECTS_LEDGER = new URL('./check_loc_rejects.txt', import.meta.url);
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
async function read_rejects_ledger(): Promise<Set<string>> {
	const text = await Deno.readTextFile(REJECTS_LEDGER);
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
 * The root as a cwd-relative path with no `./` prefix, doubled `/`, or trailing `/`, so the
 * paths `loc_wires` emits under it line up with the ledger's.
 */
function normalize_root(root: string): string {
	const cwd = `${Deno.cwd()}/`;
	let path = root.startsWith(cwd) ? root.slice(cwd.length) : root;
	path = path.replace(/\/{2,}/g, '/');
	while (path.startsWith('./')) path = path.slice(2);
	return path.replace(/\/+$/, '') || '.';
}

/** Whether a ledger entry lies under `root` — the entries a run over `root` can grade stale. */
function ledger_entry_under(path: string, root: string): boolean {
	return root === '.' || `${DEFAULT_ROOT}/${path}`.startsWith(`${root}/`);
}

async function main(): Promise<void> {
	const root = normalize_root(Deno.args[0] ?? DEFAULT_ROOT);
	const ledger = await read_rejects_ledger();
	const started = performance.now();
	const child = new Deno.Command(TSV_DEBUG, {
		args: ['loc_wires', root],
		stdout: 'piped',
		stderr: 'inherit'
	}).spawn();

	const graded: Record<Language, number> = { svelte: 0, typescript: 0, css: 0 };
	const mismatches: string[] = [];
	const rejected = new Set<string>();
	for await (const line of lines_of(child.stdout)) {
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
	const status = await child.status;
	const seconds = ((performance.now() - started) / 1000).toFixed(1);
	const total = graded.svelte + graded.typescript + graded.css;
	console.log(
		`loc cross-grade: ${total} documents (${graded.svelte} svelte, ${graded.typescript} typescript, ${graded.css} css) in ${seconds}s` +
			` — ${rejected.size} prettier-side form(s) tsv rejects, graded against check_loc_rejects.txt`
	);

	const failures: string[] = [];
	if (!status.success) failures.push(`\`${TSV_DEBUG} loc_wires\` exited ${status.code}`);
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
		Deno.exit(1);
	}
	console.log(
		'PASS: the loc wire equals the reconstruction of the span-only wire on every document'
	);
}

if (import.meta.main) await main();
