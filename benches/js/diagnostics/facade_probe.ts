/**
 * Diagnostic: what does the npm packages' facade cost per call, over the flat engine
 * exports beneath it — and how far is a bench row from the installed package?
 *
 * The published surface of every tsv npm package is one hand-written facade
 * (`crates/tsv_wasm/npm/api.js` + `api_parse.js`): per call it checks the source is a
 * well-formed UTF-16 string, reads the options bag, wraps the engine call to rethrow a
 * parse failure as the typed `SyntaxError`, and for the native package `JSON.parse`s the
 * wire inside a try/catch. The bench's `tsv` / `tsv-wasm` / `+locations` rows time that
 * facade over each binding (`lib/tsv_api.ts`). This probe prices it, per binding,
 * operation and language, under whichever runtime runs it.
 *
 * The ARMS of one cell, all over one loaded engine instance per binding:
 *
 *   - `row`      the bench row's own call, through the harness class the row uses
 *   - `control`  that same call from a second closure — the in-run A/A noise floor
 *   - `engine`   the flat exports beneath the facade (`TsvBinding.engine`), called as a
 *                caller with no facade would: the wire `JSON.parse`d bare where the
 *                engine hands back a string, `reconstruct_locations` composed by hand
 *   - `package`  (wasm only) the staged `@fuzdev/tsv-wasm` entry itself — the web-target
 *                glue plus the facade, where `row` runs this runtime's `deno` / `nodejs`
 *                target glue over the same engine
 *
 * Three readings come out of them, each a per-round RATIO taken inside one round:
 *
 *   - `floor`    control ÷ row — the row against itself
 *   - `facade`   row ÷ engine — what the facade adds (>1 = the facade costs)
 *   - `package`  package ÷ row — what separates the row from the installed package
 *
 * The `row` arm goes through the wrapper's method, as every bench row does, where
 * `engine` and `package` are called directly. That one method call is invisible on the
 * `corpus` set and a real share of the `tiny` one, where it reads as part of `facade`
 * and as `package` sitting under 1.
 *
 * The discipline is `wasm_format_probe.ts`'s: interleaved passes with the arm order
 * rotated and reversed across rounds so drift and warmup cancel, medians of per-round
 * ratios rather than absolute readings, and the floor measured in the same run. `net`
 * is a reading over the floor; one whose median sits inside the floor's `[min,max]` is
 * not distinguishable from the row calling itself twice.
 *
 * Three SETS per cell, because a fixed per-call cost is largest where the call is
 * smallest:
 *
 *   - `corpus`  the perf view's files for the language — what the report rows sweep
 *   - `small`   its smallest tenth by length, the sweep repeated to a measurable pass
 *   - `tiny`    one minimal source, repeated: the fixed cost per call in nanoseconds
 *
 * A byte-identity gate runs first: every arm must return what `row` returns on every
 * file (formats compared as strings, parses as their `JSON.stringify`), since timing
 * two arms that disagree is meaningless. It also prices the facade's one O(source)
 * step alone — `String.prototype.isWellFormed` over each language's files — beside the
 * count of files that hold a character outside Latin-1 (the strings a runtime stores
 * two-byte and so has to scan).
 *
 * Runs under all three runtimes, from the repo root, against the bench's own artifacts
 * (freshness-guarded like a `:run` task; `BENCH_STALE_OK=1` overrides):
 *
 *   node --expose-gc --disable-warning=ExperimentalWarning benches/js/diagnostics/facade_probe.ts
 *   bun --expose-gc benches/js/diagnostics/facade_probe.ts
 *   deno run --v8-flags=--expose-gc --allow-ffi --allow-read --allow-env --allow-sys \
 *     --config benches/js/deno.json benches/js/diagnostics/facade_probe.ts
 *
 * Flags: `--rounds N` (15), `--warmup N` (3), `--lang <language>`, `--binding
 * <napi|ffi|wasm>`, `--op <parse|parse+locations|format>`, `--set <corpus|small|tiny>`,
 * `--no-package`, `--json` (one JSON document on stdout; the tables go to stderr
 * either way). The `package` arm needs `deno task build:npm:all`.
 */

import { stat } from 'node:fs/promises';
import { resolve } from 'node:path';
import { argv, exit } from 'node:process';
import { pathToFileURL } from 'node:url';

import { reconstruct_locations } from '../../../crates/tsv_wasm/npm/locations.js';
import { check_executed_artifacts } from '../lib/check_artifact_freshness.ts';
import { CorpusLoader, group_by_language } from '../lib/corpus.ts';
import { NativeImplementation } from '../lib/ffi.ts';
import { NapiImplementation } from '../lib/napi.ts';
import { current_runtime, runtime_version, wasm_target } from '../lib/runtime.ts';
import type { TsvBinding } from '../lib/tsv_api.ts';
import { type Language, LANGUAGES, type SourceFile } from '../lib/types.ts';
import { WasmImplementation } from '../lib/wasm.ts';

type Binding = 'napi' | 'ffi' | 'wasm';
type Operation = 'parse' | 'parse+locations' | 'format';
type SetName = 'corpus' | 'small' | 'tiny';
type ArmName = 'row' | 'control' | 'engine' | 'package';
type ReadingName = 'floor' | 'facade' | 'package';

/** One timed call: a source in, the row's product out. */
type Call = (source: string) => unknown;

/** A package entry's published functions, by export name. */
type Package = Record<string, (source: string, options?: object) => unknown>;

const OPERATIONS: Operation[] = ['parse', 'parse+locations', 'format'];
const SETS: SetName[] = ['corpus', 'small', 'tiny'];

/** Each reading as `[name, numerator arm, denominator arm]`. */
const READINGS: ReadonlyArray<readonly [ReadingName, ArmName, ArmName]> = [
	['floor', 'control', 'row'],
	['facade', 'row', 'engine'],
	['package', 'package', 'row']
];

/** A minimal valid source per language — the `tiny` set's one call. */
const TINY_SOURCES: Record<Language, string> = {
	svelte: '<p>x</p>',
	typescript: 'x;',
	css: 'a{}'
};

/** A pass shorter than this is repeated until it is not (`small` and `tiny`). */
const MIN_PASS_MS = 25;

//
// Arguments
//

let rounds = 15;
let warmup = 3;
let lang_filter: Language | null = null;
let binding_filter: Binding | null = null;
let op_filter: Operation | null = null;
let set_filter: SetName | null = null;
let with_package = true;
let json = false;
const args = argv.slice(2);
for (let i = 0; i < args.length; i++) {
	const arg = args[i];
	if (arg === '--rounds') rounds = Number(args[++i]);
	else if (arg === '--warmup') warmup = Number(args[++i]);
	else if (arg === '--lang') lang_filter = args[++i] as Language;
	else if (arg === '--binding') binding_filter = args[++i] as Binding;
	else if (arg === '--op') op_filter = args[++i] as Operation;
	else if (arg === '--set') set_filter = args[++i] as SetName;
	else if (arg === '--no-package') with_package = false;
	else if (arg === '--json') json = true;
	else {
		console.error(`unknown argument ${arg}`);
		exit(2);
	}
}
const log = (line = ''): void => console.error(line);

//
// Bindings
//

const runtime = current_runtime();
await check_executed_artifacts();

/** One binding's arms, as factories so `row` and `control` are separate closures. */
interface BindingArms {
	binding: Binding;
	/** What the binding loaded, for the header. */
	artifact: string;
	arms: Partial<Record<ArmName, (operation: Operation, language: Language) => Call>>;
}

/** The bench rows' own calls over a harness wrapper (`lib/implementations.ts`). */
const row_call = (impl: TsvBinding, operation: Operation, language: Language): Call => {
	if (operation === 'format') return (source) => impl.format(source, language);
	if (operation === 'parse') return (source) => impl.parse(source, language);
	return (source) => impl.parse_with_locations(source, language);
};

/** The same products off the flat exports, with nothing between the caller and them. */
const engine_call = (impl: TsvBinding, operation: Operation, language: Language): Call => {
	const { engine } = impl;
	if (operation === 'format') {
		const format = engine.format[language];
		return (source) => format(source);
	}
	const parse_object = engine.parse?.[language];
	const parse_json = engine.parse_json[language];
	const parse: Call = parse_object
		? (source) => parse_object(source)
		: (source) => JSON.parse(parse_json(source));
	if (operation === 'parse') return parse;
	return (source) => reconstruct_locations(parse(source), source, { language });
};

/** A package consumer's own spelling of each operation. */
const package_call = (pkg: Package, operation: Operation, language: Language): Call => {
	if (operation === 'format') {
		const format = pkg[`format_${language}`];
		return (source) => format(source);
	}
	const parse = pkg[`parse_${language}`];
	if (operation === 'parse') return (source) => parse(source);
	return (source) => parse(source, { locations: true });
};

const harness_arms = (impl: TsvBinding): BindingArms['arms'] => ({
	row: (operation, language) => row_call(impl, operation, language),
	control: (operation, language) => row_call(impl, operation, language),
	engine: (operation, language) => engine_call(impl, operation, language)
});

const bindings: BindingArms[] = [];

if (runtime === 'deno') {
	const ffi = new NativeImplementation();
	await ffi.init();
	bindings.push({
		binding: 'ffi',
		artifact: 'tsv_ffi (release), lib/ffi.ts',
		arms: harness_arms(ffi)
	});
} else {
	const napi = new NapiImplementation();
	await napi.init();
	bindings.push({
		binding: 'napi',
		artifact: 'tsv_napi (napi profile), lib/napi.ts',
		arms: harness_arms(napi)
	});
}

{
	const wasm = new WasmImplementation();
	await wasm.init();
	const arms = harness_arms(wasm);
	let artifact = `pkg/all/${wasm_target()}, lib/wasm.ts`;
	if (with_package) {
		// The staged `@fuzdev/tsv-wasm` Node entry — its own instance of the same engine
		// behind the web-target glue. Loaded by path: the probe asks what that file does.
		const entry = resolve('crates/tsv_wasm/pkg/all/npm/index.js');
		try {
			await stat(entry);
		} catch {
			console.error(
				`staged package not found at ${entry} — run 'deno task build:npm:all', or pass --no-package`
			);
			exit(1);
		}
		const pkg = (await import(pathToFileURL(entry).href)) as Package;
		arms.package = (operation, language) => package_call(pkg, operation, language);
		artifact += ' + pkg/all/npm/index.js';
	}
	bindings.push({ binding: 'wasm', artifact, arms });
}

//
// Corpus
//

const loaded: SourceFile[] = [];
for await (const file of new CorpusLoader('perf', { missing: 'fail' }).stream(() => {})) {
	loaded.push(file);
}
const corpus = group_by_language(loaded);

const languages = LANGUAGES.filter((language) => !lang_filter || language === lang_filter);
const active_bindings = bindings.filter((b) => !binding_filter || b.binding === binding_filter);
const operations = OPERATIONS.filter((operation) => !op_filter || operation === op_filter);
const sets = SETS.filter((set) => !set_filter || set === set_filter);

/** What two arms must agree on: a format's string, or a parse's serialized tree. */
const product_text = (product: unknown): string | null => {
	if (typeof product === 'string') return product;
	try {
		// a pathologically deep tree overflows the recursive serializer — ungraded
		return JSON.stringify(product);
	} catch {
		return null;
	}
};

// The byte-identity gate, and the accept set every arm is timed over: the files `row`
// takes for every operation in scope, on which every other arm returns the same bytes.
const accepted: Record<string, SourceFile[]> = {};
for (const b of active_bindings) {
	for (const language of languages) {
		const kept: SourceFile[] = [];
		let mismatches = 0;
		let ungraded = 0;
		const calls = operations.map((operation) => ({
			row: b.arms.row!(operation, language),
			others: [b.arms.engine, b.arms.package]
				.filter((make) => make !== undefined)
				.map((make) => make(operation, language))
		}));
		for (const file of corpus[language]) {
			let ok = true;
			for (const { row, others } of calls) {
				let expected: string | null;
				try {
					expected = product_text(row(file.content));
				} catch {
					ok = false;
					break;
				}
				if (expected === null) {
					ungraded++;
					continue;
				}
				for (const other of others) {
					if (product_text(other(file.content)) !== expected) mismatches++;
				}
			}
			if (ok) kept.push(file);
		}
		accepted[`${b.binding}/${language}`] = kept;
		log(
			`identity ${b.binding}/${language}: ${kept.length}/${corpus[language].length} files, ` +
				`${mismatches} mismatch${ungraded ? `, ${ungraded} ungraded` : ''}`
		);
		if (mismatches) {
			console.error('ABORT: an arm returns different bytes than the row it is compared to.');
			exit(1);
		}
	}
}

//
// Measurement
//

const collect: (() => void) | undefined = (globalThis as { gc?: () => void }).gc;

const median = (values: number[]): number => {
	const sorted = [...values].sort((a, b) => a - b);
	const mid = Math.floor(sorted.length / 2);
	return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
};

/** One pass: `repeat` sweeps of `sources` through `call`, in milliseconds. */
const pass = (call: Call, sources: string[], repeat: number): number => {
	collect?.();
	const start = performance.now();
	for (let r = 0; r < repeat; r++) {
		for (let i = 0; i < sources.length; i++) {
			if (call(sources[i]) === undefined) throw new Error('an arm returned nothing');
		}
	}
	return performance.now() - start;
};

interface Reading {
	/** The per-round ratio: its median and extremes. */
	ratio: number;
	min: number;
	max: number;
	/** Median per-round difference between the two arms, in nanoseconds per call. */
	delta_ns: number;
}

interface CellReading {
	binding: Binding;
	operation: Operation;
	language: Language;
	set: SetName;
	files: number;
	kb: number;
	/** Calls per pass (`files` × the repeat count). */
	calls: number;
	/** The row's median cost per call, in nanoseconds. */
	row_ns: number;
	readings: Partial<Record<ReadingName, Reading>>;
}

const measure = (
	b: BindingArms,
	operation: Operation,
	language: Language,
	set: SetName
): CellReading | null => {
	const files = accepted[`${b.binding}/${language}`];
	let sources: string[];
	if (set === 'corpus') {
		sources = files.map((f) => f.content);
	} else if (set === 'small') {
		const by_length = [...files].sort((x, y) => x.content.length - y.content.length);
		sources = by_length.slice(0, Math.max(1, Math.ceil(files.length / 10))).map((f) => f.content);
	} else {
		sources = [TINY_SOURCES[language]];
	}
	if (sources.length === 0) return null;

	const arm_calls = Object.entries(b.arms).map(
		([name, make]) => [name as ArmName, make(operation, language)] as const
	);

	// size the repeat so a pass is long enough to time, from the row
	const row = arm_calls.find(([name]) => name === 'row')![1];
	let repeat = 1;
	while (pass(row, sources, repeat) < MIN_PASS_MS && repeat < 1 << 24) repeat *= 2;

	for (let w = 0; w < warmup; w++) {
		for (const [, call] of arm_calls) pass(call, sources, repeat);
	}

	const samples = new Map<ArmName, number[]>(arm_calls.map(([name]) => [name, []]));
	for (let round = 0; round < rounds; round++) {
		// rotate the order each round and reverse it every other cycle, so no arm keeps
		// a position
		const shift = round % arm_calls.length;
		const order = [...arm_calls.slice(shift), ...arm_calls.slice(0, shift)];
		if (Math.floor(round / arm_calls.length) % 2 === 1) order.reverse();
		for (const [name, call] of order) samples.get(name)!.push(pass(call, sources, repeat));
	}

	const calls = sources.length * repeat;
	const readings: CellReading['readings'] = {};
	for (const [name, numerator, denominator] of READINGS) {
		const over = samples.get(numerator);
		const under = samples.get(denominator);
		if (!over || !under) continue;
		const ratios = over.map((t, i) => t / under[i]);
		readings[name] = {
			ratio: median(ratios),
			min: Math.min(...ratios),
			max: Math.max(...ratios),
			delta_ns: (median(over.map((t, i) => t - under[i])) * 1e6) / calls
		};
	}
	return {
		binding: b.binding,
		operation,
		language,
		set,
		files: sources.length,
		kb: sources.reduce((sum, s) => sum + s.length, 0) / 1024,
		calls,
		row_ns: (median(samples.get('row')!) * 1e6) / calls,
		readings
	};
};

const fixed = (value: number, digits: number, width: number): string =>
	value.toFixed(digits).padStart(width);

const print_cell = (cell: CellReading): void => {
	const floor = cell.readings.floor!;
	let line =
		`${cell.binding.padEnd(5)} ${cell.operation.padEnd(15)} ${cell.language.padEnd(10)} ` +
		`${cell.set.padEnd(6)} ${String(cell.files).padStart(5)}f ${fixed(cell.kb, 1, 9)}KB ` +
		`row ${fixed(cell.row_ns, 0, 9)}ns/call  ` +
		`floor ${fixed(floor.ratio, 4, 6)} [${fixed(floor.min, 3, 5)},${fixed(floor.max, 3, 5)}]`;
	for (const name of ['facade', 'package'] as const) {
		const reading = cell.readings[name];
		if (!reading) continue;
		line +=
			`  ${name} ${fixed(reading.ratio, 4, 6)} net ${fixed(reading.ratio / floor.ratio, 4, 6)} ` +
			`[${fixed(reading.min, 3, 5)},${fixed(reading.max, 3, 5)}] ${fixed(reading.delta_ns, 0, 7)}ns`;
	}
	log(line);
};

log(
	`\n${runtime} ${runtime_version()} · ${rounds} rounds, warmup ${warmup}` +
		`${collect ? '' : ' · no gc() exposed'}`
);
for (const b of active_bindings) log(`  ${b.binding}: ${b.artifact}`);
log(
	`\nfloor = control÷row (the row against itself) · facade = row÷engine (what the facade ` +
		`adds over the flat exports) · package = package÷row · net = ratio÷floor · ` +
		`ns = median per-call difference\n`
);

const cells: CellReading[] = [];
for (const set of sets) {
	for (const b of active_bindings) {
		for (const operation of operations) {
			for (const language of languages) {
				const cell = measure(b, operation, language, set);
				if (!cell) continue;
				cells.push(cell);
				print_cell(cell);
			}
		}
	}
	log();
}

//
// The facade's one O(source) step, alone
//

interface WellFormedReading {
	language: Language;
	files: number;
	/** Files holding a character outside Latin-1 — stored two-byte, so actually scanned. */
	two_byte_files: number;
	two_byte_kb: number;
	/** One `isWellFormed` call per file, the whole language, in microseconds. */
	sweep_us: number;
}

const well_formed: WellFormedReading[] = [];
for (const language of languages) {
	const sources = corpus[language].map((f) => f.content);
	const two_byte = sources.filter((s) => /[^\u0000-ÿ]/.test(s));
	const scan: Call = (source) => source.isWellFormed();
	let repeat = 1;
	while (pass(scan, sources, repeat) < MIN_PASS_MS && repeat < 1 << 24) repeat *= 2;
	const timings: number[] = [];
	for (let i = 0; i < rounds; i++) timings.push(pass(scan, sources, repeat));
	const reading: WellFormedReading = {
		language,
		files: sources.length,
		two_byte_files: two_byte.length,
		two_byte_kb: two_byte.reduce((sum, s) => sum + s.length, 0) / 1024,
		sweep_us: (median(timings) * 1000) / repeat
	};
	well_formed.push(reading);
	log(
		`isWellFormed ${language.padEnd(10)} ${String(reading.files).padStart(5)}f, ` +
			`${reading.two_byte_files} two-byte (${reading.two_byte_kb.toFixed(1)}KB): ` +
			`${reading.sweep_us.toFixed(1)}us per sweep`
	);
}

if (json) {
	console.log(
		JSON.stringify({
			runtime,
			runtime_version: runtime_version(),
			rounds,
			warmup,
			cells,
			well_formed
		})
	);
}
