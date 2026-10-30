/**
 * The oracle side of the strip fixtures (`tests/fixtures_strip/`): regenerates and
 * re-checks each fixture's expected output against ts-blank-space and Node's
 * `module.stripTypeScriptTypes`. tsv's own strip is graded against the committed files by
 * `tests/strip_fixtures_tests.rs` (`cargo test`); this script is what keeps those files
 * true to the oracles.
 *
 * Per fixture (a directory holding one `input.ts` / `input.mts` / `input.svelte.ts`):
 *
 * - **The derivation.** tsv's expected output is ts-blank-space's, run over the input
 *   with every CR, U+2028 and U+2029 folded to LF, each one then restored at its UTF-16
 *   offset. ts-blank-space tests "spans lines" against LF alone, so the fold is what
 *   makes it see every ECMAScript line terminator at the hazard sites, and the restore
 *   is tsv's contract that every terminator survives verbatim. On an input holding none
 *   the derivation IS ts-blank-space's output, byte for byte.
 * - **Refusals.** When ts-blank-space reports a node through its `onError` callback the
 *   fixture is a refusal: `refusal.txt` records tsv's `UnsupportedKind` name and the
 *   node's start and end as `line:column` (1-based line, 0-based UTF-16 column,
 *   ECMAScript line terminators — tsv's `WirePoint`), the first reported node in source
 *   order.
 * - **Oracle agreement.** Each oracle's own output on the raw input is compared with the
 *   derivation: ts-blank-space byte for byte; Node modulo the whitespace it writes where
 *   the derivation writes a space (it keeps a blanked tab, and blanks a non-ASCII
 *   character to U+00A0, U+2002 or U+FEFF so the byte length holds) — never modulo a
 *   line terminator. A refusal agrees when the oracle refuses too (Node throws). Every
 *   disagreement must be declared in the fixture's `divergences.txt` (one oracle name per
 *   line: `ts-blank-space`, `node`), and every declared one must still fire — the
 *   fixture's README says why.
 * - **Overrides.** Where ts-blank-space is wrong, the line `override ts-blank-space` (in
 *   place of a plain `ts-blank-space`) makes the claim a hand-reviewed one: the committed
 *   `expected.*` or `refusal.txt` is not derived and never written, but validated — an
 *   output must keep the position contract below and pass the JS-validity guard, a refusal
 *   must name an `UnsupportedKind` and a span inside the input — and graded against both
 *   oracles like a derived claim. The derivation must still disagree with it, or the
 *   override is stale and fails. Node never derives a claim, so a plain `node` line
 *   already declares a claim that stands against Node; only ts-blank-space is overridden.
 * - **The position contract.** Every claimed output has the input's UTF-16 length, keeps
 *   every line terminator at its offset, and holds at every other offset the input's
 *   character, a space, or a `;` / `(` / `)` — the contract tsv's strip is graded by
 *   (`crates/tsv_debug/src/strip_fixtures.rs`).
 * - **JS validity.** Every claimed output must parse as a JS module under oxc-parser in JS
 *   mode (semantic errors included), or the fixture fails and nothing is written. tsv's
 *   Rust side parses the expected output with `tsv_ts::__private::parse`, which accepts TypeScript,
 *   so this is the check that the output is JS at all.
 *
 * One input can't be fixtured: a U+2028 or U+2029 inside a string literal. The fold to LF
 * makes it a line break inside the string, which TypeScript does not parse, so the run
 * fails loudly on it (the parse-diagnostics error) rather than writing a claim.
 *
 * Node is unpinned — `stripTypeScriptTypes` moves with the installed Node — so the run
 * prints the versions it graded. The derivation, the refusal record and the JS-validity
 * guard are `lib/strip_oracle.ts`, which the strip corpus differential (`strip_corpus.ts`)
 * shares.
 *
 * Usage (from the repo root; needs `deno task bench:install` and Node ≥ 22.18):
 *
 *   deno task strip:fixtures:oracle [pattern...]   re-check the committed files
 *   deno task strip:fixtures:update [pattern...]   write expected.* / refusal.txt (an
 *                                                  override fixture's are never written)
 *
 * Exit codes: 0 = every fixture agrees with its oracles, 1 = any mismatch.
 */

import * as node_module from 'node:module';
import { existsSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { argv, exit, versions } from 'node:process';
import { fileURLToPath } from 'node:url';

import { check_node_modules } from './lib/check_node_modules.ts';

const repo_root = join(dirname(fileURLToPath(import.meta.url)), '../..');
const FIXTURES_DIR = 'tests/fixtures_strip';

/** Each input file name and the expected-output name its stripped JS takes. */
const INPUTS: ReadonlyArray<readonly [input: string, expected: string]> = [
	['input.ts', 'expected.js'],
	['input.mts', 'expected.mjs'],
	['input.svelte.ts', 'expected.svelte.js']
];
const REFUSAL_FILE = 'refusal.txt';
const DIVERGENCES_FILE = 'divergences.txt';
const ORACLES = ['ts-blank-space', 'node'] as const;
type Oracle = (typeof ORACLES)[number];
/** The `divergences.txt` line that makes the claim a hand-reviewed one (module docs). */
const OVERRIDE_LINE = 'override ts-blank-space';

if ('Deno' in globalThis) {
	// `deno task` can put a `node` shim on PATH that runs this file AS Deno, whose
	// `node:module` has no `stripTypeScriptTypes` to grade against
	console.error('Error: strip_fixtures.ts must run under Node, not Deno.');
	exit(1);
}
if (typeof node_module.stripTypeScriptTypes !== 'function') {
	console.error(`Error: this Node (${versions.node}) has no module.stripTypeScriptTypes.`);
	exit(1);
}

await check_node_modules();
// Dynamic so a missing node_modules reports through the preflight above.
const {
	derive,
	is_terminator,
	js_parse_failure,
	ORACLE_VERSIONS,
	point,
	run_ts_blank_space,
	UNSUPPORTED_KINDS
} = await import('./lib/strip_oracle.ts');
type Outcome = import('./lib/strip_oracle.ts').Outcome;

const args = argv.slice(2);
const update = args.includes('--update');
const filters = args.filter((a) => !a.startsWith('--')).map((a) => a.toLowerCase());

interface Fixture {
	dir: string;
	relative: string;
	input: string;
	expected: string;
}

/** Walk the tree: every directory is a fixture (one input file) or a container. */
function walk(dir: string, relative: string, out: Array<Fixture>, errors: Array<string>): void {
	const entries = readdirSync(dir, { withFileTypes: true });
	const subdirs = entries.filter((e) => e.isDirectory()).map((e) => e.name);
	const inputs = INPUTS.filter(([input]) => entries.some((e) => e.isFile() && e.name === input));
	if (inputs.length > 1) {
		errors.push(`${relative}: more than one input file`);
		return;
	}
	if (inputs.length === 1 && subdirs.length > 0) {
		errors.push(`${relative}: has both an input file and subdirectories`);
		return;
	}
	const [only] = inputs;
	if (only) {
		out.push({ dir, relative, input: only[0], expected: only[1] });
		return;
	}
	if (subdirs.length === 0 && relative !== '') {
		errors.push(`${relative}: neither an input file nor subdirectories (orphan)`);
		return;
	}
	for (const name of subdirs.sort()) {
		walk(join(dir, name), relative ? `${relative}/${name}` : name, out, errors);
	}
}

/**
 * What `output` breaks of the position contract against `input`, or `null`: the same
 * UTF-16 length, every line terminator at its offset, and at every other offset the input's
 * character, a space, or a `;` / `(` / `)`. The Rust side grades every committed output
 * by the same contract (`contract_violations`).
 */
function contract_failure(input: string, output: string): string | null {
	if (output.length !== input.length) {
		return `UTF-16 length ${input.length} became ${output.length}`;
	}
	for (let i = 0; i < input.length; i++) {
		const ours = input[i]!;
		const theirs = output[i]!;
		if (ours === theirs) continue;
		const at = `${point(input, i)} (UTF-16 offset ${i})`;
		if (is_terminator(ours) || is_terminator(theirs)) {
			return `a line terminator moved at ${at}`;
		}
		if (theirs !== ' ' && theirs !== ';' && theirs !== '(' && theirs !== ')') {
			return `${JSON.stringify(ours)} became ${JSON.stringify(theirs)} at ${at}, neither kept nor a blank or a \`;\` / \`(\` / \`)\``;
		}
	}
	return null;
}

/** The UTF-16 offset of `line:column` in `text` (tsv's `WirePoint`), or `null` past its end. */
function offset_of(text: string, line: number, column: number): number | null {
	let start = 0;
	for (let l = 1; l < line; l++) {
		let i = start;
		while (i < text.length && !is_terminator(text[i]!)) i++;
		if (i === text.length) return null;
		start = text[i] === '\r' && text[i + 1] === '\n' ? i + 2 : i + 1;
	}
	let end = start;
	while (end < text.length && !is_terminator(text[end]!)) end++;
	return column <= end - start ? start + column : null;
}

/**
 * What is wrong with a hand-reviewed `refusal.txt`, or `null`: one line
 * `Kind<TAB>line:column<TAB>line:column`, the kind an `UnsupportedKind`, both points inside
 * the input and in order.
 */
function refusal_failure(input: string, refusal: string): string | null {
	const match = /^([A-Za-z]+)\t(\d+):(\d+)\t(\d+):(\d+)\n$/.exec(refusal);
	if (!match) return 'malformed — expected `Kind<TAB>line:column<TAB>line:column`';
	const [, kind, start_line, start_column, end_line, end_column] = match;
	if (!(UNSUPPORTED_KINDS as ReadonlyArray<string>).includes(kind!)) {
		return `${kind} is no UnsupportedKind (${UNSUPPORTED_KINDS.join(', ')})`;
	}
	const start = offset_of(input, Number(start_line), Number(start_column));
	const end = offset_of(input, Number(end_line), Number(end_column));
	if (start === null || end === null) return 'a span end lies outside the input';
	if (start >= end) return 'the span is empty or ends before it starts';
	return null;
}

/** Node's output, or its refusal (it throws). */
function run_node(input: string): Outcome {
	try {
		return { code: node_module.stripTypeScriptTypes(input, { mode: 'strip' }) };
	} catch (e) {
		return { refusal: (e instanceof Error ? e.message.split('\n')[0] : undefined) ?? 'refused' };
	}
}

/**
 * Whether `outcome` agrees with the claim `derived`. For ts-blank-space: the same refusal
 * record, or the same code byte for byte. For Node (`whitespace_lenient`): any refusal (it
 * throws, naming no node), or the same code modulo the whitespace it writes where the claim
 * writes a space — a kept tab, or a non-ASCII substitute (U+00A0, U+2002, U+FEFF) that
 * holds the byte length. A line terminator is never lenient.
 */
function agrees(outcome: Outcome, derived: Outcome, whitespace_lenient: boolean): boolean {
	if ('refusal' in derived) {
		return 'refusal' in outcome && (whitespace_lenient || outcome.refusal === derived.refusal);
	}
	if (!('code' in outcome)) return false;
	if (outcome.code === derived.code) return true;
	if (!whitespace_lenient || outcome.code.length !== derived.code.length) return false;
	for (let i = 0; i < derived.code.length; i++) {
		const theirs = outcome.code[i]!;
		const ours = derived.code[i]!;
		if (theirs === ours) continue;
		if (ours !== ' ' || is_terminator(theirs) || !/\s/.test(theirs)) return false;
	}
	return true;
}

/** How many UTF-16 units of Node's code differ from the derived code (whitespace only, once agreed). */
function count_differing(outcome: Outcome, derived: Outcome): number {
	if (!('code' in outcome) || !('code' in derived)) return 0;
	let n = 0;
	for (let i = 0; i < derived.code.length; i++) if (outcome.code[i] !== derived.code[i]) n++;
	return n;
}

/** A fixture's `divergences.txt`: the oracles it departs from, and whether it overrides. */
interface Declared {
	oracles: Array<Oracle>;
	override: boolean;
}

function read_declared(dir: string, fail: (message: string) => void): Declared | null {
	const path = join(dir, DIVERGENCES_FILE);
	if (!existsSync(path)) return null;
	const lines = readFileSync(path, 'utf8')
		.split('\n')
		.map((l) => l.trim())
		.filter((l) => l !== '');
	if (lines.length === 0) fail(`empty ${DIVERGENCES_FILE}`);
	const declared: Declared = { oracles: [], override: false };
	for (const line of lines) {
		const override = line === OVERRIDE_LINE;
		const name = override ? 'ts-blank-space' : line;
		if (line.startsWith('override ') && !override) {
			fail(
				`${DIVERGENCES_FILE}: only ts-blank-space, the oracle a claim is derived from, can be ` +
					`overridden — declare a departure from Node as a plain \`node\` line: ${line}`
			);
		} else if (!(ORACLES as ReadonlyArray<string>).includes(name)) {
			fail(`${DIVERGENCES_FILE} names an unknown oracle: ${line}`);
		} else if (declared.oracles.includes(name as Oracle)) {
			fail(`${DIVERGENCES_FILE} names ${name} twice`);
		} else {
			declared.oracles.push(name as Oracle);
			declared.override ||= override;
		}
	}
	return declared;
}

/** A hand-reviewed claim, read from disk: exactly one of `expected.*` and `refusal.txt`. */
function read_reviewed_claim(fixture: Fixture, fail: (message: string) => void): Outcome | null {
	const expected_path = join(fixture.dir, fixture.expected);
	const refusal_path = join(fixture.dir, REFUSAL_FILE);
	const has_expected = existsSync(expected_path);
	const has_refusal = existsSync(refusal_path);
	if (has_expected === has_refusal) {
		fail(
			`an override fixture commits exactly one hand-reviewed claim, ${fixture.expected} or ` +
				`${REFUSAL_FILE} — found ${has_expected ? 'both' : 'neither'}`
		);
		return null;
	}
	return has_expected
		? { code: readFileSync(expected_path, 'utf8') }
		: { refusal: readFileSync(refusal_path, 'utf8') };
}

const ALLOWED_FILES = new Set([...INPUTS.flat(), REFUSAL_FILE, DIVERGENCES_FILE, 'README.md']);

const root = join(repo_root, FIXTURES_DIR);
const fixtures: Array<Fixture> = [];
const failures: Array<string> = [];
walk(root, '', fixtures, failures);
const selected = fixtures.filter(
	(f) => filters.length === 0 || filters.some((p) => f.relative.toLowerCase().includes(p))
);
if (selected.length === 0) {
	console.error(`Error: no strip fixtures matched under ${FIXTURES_DIR}`);
	exit(1);
}

console.log(
	`strip oracles: ts-blank-space ${ORACLE_VERSIONS.ts_blank_space} (typescript ${ORACLE_VERSIONS.typescript}), ` +
		`Node ${versions.node} module.stripTypeScriptTypes; JS parse guard: oxc-parser ` +
		ORACLE_VERSIONS.oxc_parser
);

let outputs = 0;
let refusals = 0;
let written = 0;
let overrides = 0;
const declared_total: Record<Oracle, number> = { 'ts-blank-space': 0, node: 0 };

for (const fixture of selected) {
	const fail = (message: string): void => {
		failures.push(`${fixture.relative}: ${message}`);
	};
	for (const name of readdirSync(fixture.dir)) {
		if (!ALLOWED_FILES.has(name) || statSync(join(fixture.dir, name)).isDirectory()) {
			fail(`unexpected file ${name}`);
		}
	}
	const input = readFileSync(join(fixture.dir, fixture.input), 'utf8');
	const declared = read_declared(fixture.dir, fail);
	const reviewed = declared?.override ?? false;
	let derived: Outcome;
	let raw: Outcome;
	try {
		derived = derive(input);
		raw = run_ts_blank_space(input, input);
	} catch (e) {
		fail(e instanceof Error ? e.message : String(e));
		continue;
	}
	const node = run_node(input);

	// the claim: the derivation, or under an override the committed hand-reviewed one
	let claim: Outcome;
	if (reviewed) {
		const committed = read_reviewed_claim(fixture, fail);
		if (committed === null) continue;
		claim = committed;
		overrides++;
	} else {
		claim = derived;
	}

	// the validity guards, ahead of any write: no output that is no JS or breaks the
	// position contract, and no refusal naming no kind or no span of the input, is claimed
	if ('code' in claim) {
		const failure = js_parse_failure(claim.code, fixture.expected);
		if (failure !== null) {
			fail(
				`the ${reviewed ? 'reviewed' : 'derived'} ${fixture.expected} does not parse as a ` +
					`JS module: ${failure}` +
					(reviewed
						? ''
						: ` — if ts-blank-space is wrong here, commit a reviewed claim under ` +
							`\`${OVERRIDE_LINE}\` (tests/fixtures_strip/README.md)`)
			);
			continue;
		}
		const contract = contract_failure(input, claim.code);
		if (contract !== null) {
			fail(
				`the ${reviewed ? 'reviewed' : 'derived'} ${fixture.expected} breaks the contract: ${contract}`
			);
			continue;
		}
	} else {
		const failure = refusal_failure(input, claim.refusal);
		if (failure !== null) {
			fail(`the ${reviewed ? 'reviewed' : 'derived'} ${REFUSAL_FILE}: ${failure}`);
			continue;
		}
	}
	if ('refusal' in claim) refusals++;
	else outputs++;

	// the committed claim: expected.* for an output, refusal.txt for a refusal — derived
	// claims only; a reviewed one is the committed file itself
	if (!reviewed) {
		const expected_path = join(fixture.dir, fixture.expected);
		const refusal_path = join(fixture.dir, REFUSAL_FILE);
		const [claim_path, claim_text, other_path] =
			'refusal' in claim
				? [refusal_path, claim.refusal, expected_path]
				: [expected_path, claim.code, refusal_path];
		if (update) {
			const current = existsSync(claim_path) ? readFileSync(claim_path, 'utf8') : null;
			if (current !== claim_text) {
				writeFileSync(claim_path, claim_text);
				written++;
			}
			if (existsSync(other_path)) {
				rmSync(other_path);
				written++;
			}
		} else {
			if (!existsSync(claim_path)) {
				fail(`missing ${claim_path.slice(fixture.dir.length + 1)} — run strip:fixtures:update`);
			} else if (readFileSync(claim_path, 'utf8') !== claim_text) {
				fail(
					`stale ${claim_path.slice(fixture.dir.length + 1)} — the oracle now derives:\n${claim_text}`
				);
			}
			if (existsSync(other_path)) {
				fail(`stray ${other_path.slice(fixture.dir.length + 1)} beside the claim`);
			}
		}
	}

	// oracle agreement, against the declared divergences: ts-blank-space is graded raw against
	// a derived claim, and by its derivation against a reviewed one (the override is stale
	// once the derivation produces the claim)
	const observed: Array<Oracle> = [];
	const tbs = reviewed ? derived : raw;
	if (!agrees(tbs, claim, false)) observed.push('ts-blank-space');
	const node_agrees = agrees(node, claim, true);
	if (!node_agrees) observed.push('node');
	for (const oracle of ORACLES) {
		const is_declared = declared?.oracles.includes(oracle) ?? false;
		const fires = observed.includes(oracle);
		if (is_declared) declared_total[oracle]++;
		const line = oracle === 'ts-blank-space' && reviewed ? OVERRIDE_LINE : oracle;
		if (fires && !is_declared) {
			const theirs = oracle === 'node' ? node : tbs;
			fail(
				`undeclared ${oracle} divergence:\n  claim: ${show(claim)}\n  ${oracle}: ${show(theirs)}`
			);
		} else if (is_declared && !fires) {
			fail(
				`stale ${DIVERGENCES_FILE} entry \`${line}\`: ${oracle} now ` +
					(reviewed && oracle === 'ts-blank-space'
						? 'derives the reviewed claim — drop the override and regenerate'
						: 'agrees')
			);
		}
	}
	const notes: Array<string> = [];
	if (reviewed) notes.push('reviewed claim');
	if (observed.length > 0) notes.push(`diverges from ${observed.join(', ')}`);
	const lenient = node_agrees ? count_differing(node, claim) : 0;
	if (lenient > 0) notes.push(`Node writes ${lenient} other whitespace character(s)`);
	console.log(
		`  ${'refusal' in claim ? 'refuses' : 'strips '} ${fixture.relative}` +
			(notes.length > 0 ? `  [${notes.join('; ')}]` : '')
	);
}

function show(outcome: Outcome): string {
	return 'refusal' in outcome
		? `refuses (${outcome.refusal.trim()})`
		: JSON.stringify(outcome.code);
}

console.log(
	`\n${selected.length} fixture(s): ${outputs} output(s), ${refusals} refusal(s), ` +
		`${overrides} reviewed under an override; declared divergences: ts-blank-space ` +
		`${declared_total['ts-blank-space']}, node ${declared_total.node}` +
		(update ? `; ${written} file(s) written` : '')
);
if (failures.length > 0) {
	console.error(`\n✗ ${failures.length} failure(s):`);
	for (const f of failures) console.error(`  ${f}`);
	exit(1);
}
console.log('✓ every fixture agrees with its oracles');
