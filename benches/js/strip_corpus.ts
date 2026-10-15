/**
 * The strip corpus differential — `deno task strip:corpus`: tsv's source type strip
 * (`tsv_ts::strip`) against ts-blank-space over real code.
 *
 * **Corpus.** Every `.ts` / `.mts` / `.cts` file (`.svelte.ts` included) of the `gates` view's
 * snapshot tiers — the `../corpora` collections, pinned by tree id — less declaration files
 * (`.d.ts`, `.d.mts`, `.d.cts`), which are ambient: there is no JS to strip them to, and
 * blanking one leaves `export const x;`. Not the view's Prettier suites: they are formatter
 * fixtures rather than programs (redeclarations, multiple default exports, grammar errors tsc
 * reports from its checker), so whether a strip of one is valid JS, or refused, says nothing
 * about the strip. Not the `robustness` view: its live working trees are unpinned, and the
 * counts here are pinned.
 *
 * **tsv's side** is one `tsv_debug strip --stdin` process (the `--profile corpus` build
 * `deno task build:check` makes, so a panic is caught and answered as one), which also grades
 * what needs tsv to grade: the position contract, idempotence, and the no-copy promise (the
 * command's module doc, `crates/tsv_debug/src/cli/commands/strip.rs`).
 *
 * **Per file**, failing the run:
 *
 * - a panic, a tsv parse error, or a contract / idempotence / copy finding — never accepted;
 * - TypeScript's parser reporting a diagnostic, since ts-blank-space then grades nothing;
 * - an output that is not a JS module under oxc-parser in JS mode, semantic errors included;
 * - an output that is not ts-blank-space's byte for byte (the fixtures' derivation:
 *   `lib/strip_oracle.ts`, ts-blank-space itself on a file holding no CR, U+2028 or U+2029);
 * - a refusal ts-blank-space does not make, or one it makes that tsv does not, or the two
 *   records (kind and span) differing.
 *
 * The last three take an `ACCEPTED` entry below, each naming the file, the check, and why
 * tsv's answer stands. **The run** also fails on an accepted entry that no longer fires, on
 * an empty corpus, and on a count off its pin (`STRIP_CORPUS_PINS`).
 *
 * Takes no arguments: the whole corpus strips in seconds, so every run is a full one. To
 * look at one file, `tsv_debug strip <path>`.
 *
 * @module
 */

import { relative, resolve } from 'node:path';
import process from 'node:process';

import { check_artifact_freshness } from './lib/check_artifact_freshness.ts';
import { check_node_modules } from './lib/check_node_modules.ts';
import { CORPORA_COLLECTIONS, is_collection_path } from './lib/corpora.ts';
import { CorpusLoader } from './lib/corpus.ts';
import { STRIP_CORPUS_PINS } from './lib/gate_counts.ts';
import { TSV_DEBUG_CORPUS } from './lib/loc_wire_client.ts';
import { lines_of } from './lib/text_lines.ts';

/** A check an `ACCEPTED` entry can excuse. */
type AcceptedCheck = 'output' | 'refusal' | 'js';

/**
 * The reviewed departures: one file (relative to `../corpora/collections`), the check that
 * fires on it, and why tsv's answer stands. Each must still fire, or the run fails.
 */
const ACCEPTED: ReadonlyArray<{ path: string; check: AcceptedCheck; why: string }> = [
	{
		path: 'language-tools/packages/language-server/src/plugins/typescript-go/types.ts',
		check: 'js',
		why:
			're-exports two `import type` bindings with a plain `export { … }`, which tsc rejects ' +
			'under `verbatimModuleSyntax` (TS1448): erasure keeps the export of a binding that no ' +
			'longer exists, as ts-blank-space and Node do — the input is not erasable TypeScript'
	}
];

/** Strippable TypeScript; a declaration file is not. */
const STRIPPABLE = /\.[cm]?ts$/;
const DECLARATION = /\.d\.[cm]?ts$/;

/** One reply line of `tsv_debug strip --stdin`. */
type StripReply =
	| { code: string; findings: Array<string> }
	| { refusal: string }
	| { error: string }
	| { panic: string };

if (!('Deno' in globalThis)) {
	console.error('Error: strip_corpus.ts runs under Deno (`deno task strip:corpus`).');
	process.exit(1);
}

await check_node_modules();
await check_artifact_freshness([
	{
		label: 'tsv_debug (corpus)',
		path: TSV_DEBUG_CORPUS,
		binding_crates: ['tsv_debug', 'tsv_cli'],
		rebuild: 'deno task build:check'
	}
]);
// dynamic, so a missing node_modules reports through the preflight above
const { derive, js_parse_failure, ORACLE_VERSIONS } = await import('./lib/strip_oracle.ts');

const started = performance.now();
const collections_root = resolve(CORPORA_COLLECTIONS);
const files = (await new CorpusLoader('gates', { only: is_collection_path }).load(() => {})).filter(
	(f) => STRIPPABLE.test(f.path) && !DECLARATION.test(f.path)
);

console.log(
	`strip corpus: tsv_ts::strip vs ts-blank-space ${ORACLE_VERSIONS.ts_blank_space} ` +
		`(typescript ${ORACLE_VERSIONS.typescript}); JS guard oxc-parser ` +
		`${ORACLE_VERSIONS.oxc_parser}; ${files.length} file(s) from ${CORPORA_COLLECTIONS}`
);

const child = new Deno.Command(TSV_DEBUG_CORPUS, {
	args: ['strip', '--stdin'],
	stdin: 'piped',
	stdout: 'piped',
	stderr: 'inherit'
}).spawn();
const writer = child.stdin.getWriter();
const replies = lines_of(child.stdout);
const encoder = new TextEncoder();

const failures: Array<string> = [];
const accepted_fired = new Set<string>();
const refusal_kinds = new Map<string, number>();
let outputs = 0;
let changed = 0;
let refusals = 0;

for (const file of files) {
	const path = relative(collections_root, file.path);
	// a lone surrogate has no UTF-8 spelling, so both sides read the replaced string
	const source = file.content.toWellFormed();
	await writer.write(encoder.encode(JSON.stringify({ source }) + '\n'));
	const line = (await replies.next()).value;
	if (line === undefined) {
		console.error(`✗ ${TSV_DEBUG_CORPUS} strip --stdin exited without answering ${path}`);
		process.exit(1);
	}
	const reply = JSON.parse(line) as StripReply;
	const fail = (check: AcceptedCheck | null, message: string): void => {
		const key = `${check}\t${path}`;
		if (check !== null && ACCEPTED.some((a) => a.check === check && a.path === path)) {
			accepted_fired.add(key);
			return;
		}
		failures.push(`${path}: ${message}`);
	};

	if ('panic' in reply) {
		fail(null, `tsv panicked: ${reply.panic}`);
		continue;
	}
	if ('error' in reply) {
		fail(null, `tsv's parser rejects it: ${reply.error.split('\n')[0]}`);
		continue;
	}
	let theirs;
	try {
		theirs = derive(source);
	} catch (e) {
		fail(null, `ts-blank-space cannot grade it: ${e instanceof Error ? e.message : e}`);
		continue;
	}

	if ('refusal' in reply) {
		refusals++;
		const kind = reply.refusal.split('\t')[0]!;
		refusal_kinds.set(kind, (refusal_kinds.get(kind) ?? 0) + 1);
		if (!('refusal' in theirs)) {
			fail('refusal', `tsv refuses (${reply.refusal.trim()}), ts-blank-space strips it`);
		} else if (theirs.refusal !== reply.refusal) {
			fail(
				'refusal',
				`tsv refuses ${reply.refusal.trim()}, ts-blank-space ${theirs.refusal.trim()}`
			);
		}
		continue;
	}

	outputs++;
	if (reply.code !== source) changed++;
	for (const finding of reply.findings) fail(null, finding);
	const invalid = js_parse_failure(reply.code, file.path);
	if (invalid !== null) fail('js', `the output is not a JS module: ${invalid}`);
	if ('refusal' in theirs) {
		fail('refusal', `ts-blank-space refuses (${theirs.refusal.trim()}), tsv strips it`);
	} else if (theirs.code !== reply.code) {
		fail('output', `differs from ts-blank-space: ${first_difference(theirs.code, reply.code)}`);
	}
}

await writer.close();
const status = await child.status;
if (!status.success) failures.push(`${TSV_DEBUG_CORPUS} strip --stdin exited ${status.code}`);

if (files.length === 0) failures.push('the corpus is empty — nothing was graded');
for (const a of ACCEPTED) {
	if (!accepted_fired.has(`${a.check}\t${a.path}`)) {
		failures.push(`stale ACCEPTED entry (${a.check}) ${a.path}: it no longer fires`);
	}
}
const counts = { files: files.length, refusals };
for (const key of Object.keys(STRIP_CORPUS_PINS) as Array<keyof typeof STRIP_CORPUS_PINS>) {
	if (counts[key] !== STRIP_CORPUS_PINS[key]) {
		failures.push(
			`pinned count mismatch: ${key} is ${counts[key]}, pinned ${STRIP_CORPUS_PINS[key]} ` +
				'(benches/js/lib/gate_counts.ts STRIP_CORPUS_PINS; docs/gate_counts.md §Update ritual)'
		);
	}
}

const kinds = [...refusal_kinds].map(([kind, n]) => `${kind} ${n}`).join(', ');
const seconds = ((performance.now() - started) / 1000).toFixed(1);
console.log(
	`${files.length} file(s): ${outputs} stripped (${changed} changed), ${refusals} refused` +
		(kinds ? ` (${kinds})` : '') +
		`, ${accepted_fired.size} accepted departure(s) — ${seconds}s`
);
if (failures.length > 0) {
	console.error(`\n✗ ${failures.length} failure(s):`);
	for (const f of failures) console.error(`  ${f}`);
	process.exit(1);
}
console.log('✓ tsv strips the corpus as ts-blank-space does, but for the accepted departures');

/** The first line where `actual` departs from `expected`, both escaped. */
function first_difference(expected: string, actual: string): string {
	const a = expected.split('\n');
	const b = actual.split('\n');
	for (let i = 0; i < Math.max(a.length, b.length); i++) {
		if (a[i] !== b[i]) {
			return `line ${i + 1}: ts-blank-space ${JSON.stringify(a[i] ?? '')}, tsv ${JSON.stringify(b[i] ?? '')}`;
		}
	}
	return 'the outputs differ';
}
