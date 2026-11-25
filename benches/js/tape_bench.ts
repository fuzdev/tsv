/**
 * The tape decoder's bench — `npm run tape:bench`: the generated decoder
 * (`crates/tsv_wasm/npm/tape_decode.js`, at its full shape-list size) against `JSON.parse` of the
 * same wire, per language, over the `gates` view's snapshot collections; and the transcoder's
 * time to write those tapes.
 *
 * **Warm**: every file's span-only wire, as JSON text and as the transcoder's tape, held in
 * memory; each pass times one sweep of `JSON.parse` over every text and one of `decode_tape`
 * over every tape (the iterative interpreter alone, `decode_tape_iterative`, beside them for
 * scale), the arms' order alternating pass to pass; the report is each arm's median sweep and
 * its throughput over the JSON's bytes. Neither arm pays the boundary a binding would (a JSON
 * string or a tape crossing from the engine), so this prices the decode alone.
 *
 * **Cold**: one mid-sized file per language, each arm timed as the first call in a fresh Node
 * process, the decoder's module load timed apart from its first decode and with it. The process
 * pays its first young-generation collection before the clock starts: it costs milliseconds and
 * would land in whichever phase first crosses the initial heap limit.
 *
 * **Transcoder**: the fastest of a few transcodes of each wire, measured inside the
 * `tsv_debug tape --stdin` process — the `--profile corpus` build (optimized, no LTO), not a
 * shipped binary's profile, so a relative figure, not a shipped one.
 *
 * One machine, one sitting: read the ratios, not the absolute times.
 *
 * Runs under Node (`benches/js/roots/tape_bench.ts`): its reach is the corpus, read through the
 * corpus loader, the checkout's `target/corpus/tsv_debug`, a scratch directory for the cold
 * arm's inputs, and fresh Node processes (`TapeBenchDeps`). It writes nothing else.
 *
 * @module
 */

import { relative, resolve } from 'node:path';

import { decode_tape, decode_tape_iterative } from '../../crates/tsv_wasm/npm/tape_decode.js';
import { wire_source } from '../../scripts/check_tape.ts';
import { CORPORA_COLLECTIONS, is_collection_path } from './lib/corpora.ts';
import { CorpusLoader } from './lib/corpus.ts';
import { type TsvDebugProgram, TsvDebugServer } from './lib/tsv_debug_server.ts';
import type { TapeReply, TapeRequest } from './tape_corpus.ts';

/** What `run_tape_bench` reaches beyond the corpus loader's reads. */
export interface TapeBenchDeps {
	/** Makes the checkout's `target/corpus/tsv_debug`, called once its freshness guard passed. */
	open_tsv_debug: () => TsvDebugProgram;
	/** Writes the cold arm's inputs to a scratch directory, returning their paths. */
	write_scratch: (name: string, bytes: Uint8Array | string) => Promise<string>;
	/** Runs `code` as an ES module in a fresh Node process, resolving its stdout. */
	run_node: (code: string) => Promise<string>;
	/** Collects garbage between sweeps, when the process exposes it. */
	gc?: () => void;
}

type Language = 'svelte' | 'typescript' | 'css';

interface Doc {
	path: string;
	json: string;
	tape: Uint8Array;
	source: string;
}

/** Timed passes per arm. */
const PASSES = 15;
/** Fresh processes per cold figure. */
const COLD_RUNS = 7;
/** Transcodes per wire, the fastest kept. */
const TRANSCODE_REPEAT = 5;

const median = (values: number[]): number => {
	const sorted = [...values].sort((a, b) => a - b);
	return sorted[Math.floor(sorted.length / 2)]!;
};

const mb_per_s = (bytes: number, ms: number): string => (bytes / 1e6 / (ms / 1e3)).toFixed(0);

/**
 * The bench, printing its report.
 * @returns the exit code: `1` when the corpus is empty or a decode disagrees with
 * `JSON.parse`, else `0`
 */
export async function run_tape_bench(deps: TapeBenchDeps): Promise<number> {
	const server = await TsvDebugServer.start<TapeRequest, TapeReply>('tape', deps.open_tsv_debug);
	const collections_root = resolve(CORPORA_COLLECTIONS);
	const files = await new CorpusLoader('gates', { only: is_collection_path }).load(() => {});
	const docs: Record<Language, Doc[]> = { svelte: [], typescript: [], css: [] };
	const transcode_ns: Record<Language, number> = { svelte: 0, typescript: 0, css: 0 };
	for (const file of files) {
		const source = file.content.toWellFormed();
		const reply = await server.ask({
			language: file.language,
			source,
			repeat: TRANSCODE_REPEAT
		});
		if (!('span' in reply)) continue;
		let json: string;
		try {
			json = JSON.stringify(reply.span);
		} catch {
			continue; // a tree too deep for the recursive stringify
		}
		docs[file.language].push({
			path: relative(collections_root, file.path),
			json,
			tape: new Uint8Array(Buffer.from(reply.tape, 'base64')),
			source: wire_source(file.language, source)
		});
		transcode_ns[file.language] += reply.transcode_ns ?? 0;
	}
	await server.close();
	const total = docs.svelte.length + docs.typescript.length + docs.css.length;
	if (total === 0) {
		console.error('✗ the corpus is empty — nothing was measured');
		return 1;
	}
	// every decode agrees with JSON.parse before anything is timed
	for (const list of Object.values(docs)) {
		for (const doc of list) {
			const want = doc.json;
			let got: string;
			try {
				got = JSON.stringify(decode_tape(doc.tape, doc.source));
			} catch {
				continue;
			}
			if (got !== want) {
				console.error(`✗ ${doc.path}: the decoder disagrees with JSON.parse`);
				return 1;
			}
		}
	}
	console.log(
		`tape bench: ${total} files from ${CORPORA_COLLECTIONS} (${docs.svelte.length} svelte, ${docs.typescript.length} typescript, ${docs.css.length} css); ${PASSES} passes, median sweep`
	);
	console.log('');
	console.log(
		'| language | JSON MB | tape MB | JSON.parse ms | decode_tape ms | decode/parse | iterative ms | transcode ms (MB/s) |'
	);
	console.log('| --- | --- | --- | --- | --- | --- | --- | --- |');
	let sink = 0;
	for (const language of ['typescript', 'svelte', 'css'] as const) {
		const list = docs[language];
		if (list.length === 0) continue;
		const arms: Record<string, () => void> = {
			parse: () => {
				for (const doc of list) sink += (JSON.parse(doc.json) as { end?: number }).end ?? 0;
			},
			decode: () => {
				for (const doc of list)
					sink += (decode_tape(doc.tape, doc.source) as { end?: number }).end ?? 0;
			},
			iterative: () => {
				for (const doc of list) {
					sink += (decode_tape_iterative(doc.tape, doc.source) as { end?: number }).end ?? 0;
				}
			}
		};
		const times: Record<string, number[]> = { parse: [], decode: [], iterative: [] };
		const order = Object.keys(arms);
		for (let pass = 0; pass < PASSES + 2; pass++) {
			const this_pass = pass % 2 === 0 ? order : [...order].reverse();
			for (const arm of this_pass) {
				deps.gc?.();
				const started = performance.now();
				arms[arm]!();
				const ms = performance.now() - started;
				if (pass >= 2) times[arm]!.push(ms); // two warmup passes
			}
		}
		const json_bytes = list.reduce((n, doc) => n + Buffer.byteLength(doc.json), 0);
		const tape_bytes = list.reduce((n, doc) => n + doc.tape.length, 0);
		const parse = median(times.parse!);
		const decode = median(times.decode!);
		const iterative = median(times.iterative!);
		const transcode = transcode_ns[language] / 1e6;
		console.log(
			`| ${language} | ${(json_bytes / 1e6).toFixed(1)} | ${(tape_bytes / 1e6).toFixed(1)} | ${parse.toFixed(1)} (${mb_per_s(json_bytes, parse)} MB/s) | ${decode.toFixed(1)} (${mb_per_s(json_bytes, decode)} MB/s) | ${(decode / parse).toFixed(2)} | ${iterative.toFixed(1)} | ${transcode.toFixed(1)} (${mb_per_s(json_bytes, transcode)}) |`
		);
	}
	console.log('');
	console.log('cold, first call in a fresh process (median of ' + COLD_RUNS + '):');
	const decoder_url = new URL('../../crates/tsv_wasm/npm/tape_decode.js', import.meta.url).href;
	for (const language of ['typescript', 'svelte', 'css'] as const) {
		const list = [...docs[language]].sort((a, b) => a.json.length - b.json.length);
		if (list.length === 0) continue;
		const doc = list[Math.floor(list.length / 2)]!;
		const json_path = await deps.write_scratch(`${language}.json`, doc.json);
		const tape_path = await deps.write_scratch(`${language}.tape`, doc.tape);
		const source_path = await deps.write_scratch(`${language}.src`, doc.source);
		const parse_runs: number[] = [];
		const import_runs: number[] = [];
		const decode_runs: number[] = [];
		const total_runs: number[] = [];
		for (let run = 0; run < COLD_RUNS; run++) {
			const parse_out = await deps.run_node(`
				import { readFileSync } from 'node:fs';
				// a fresh V8 process's first young-generation collection costs milliseconds and lands in
				// whichever phase crosses the initial heap limit: pay it before the clock starts
				let junk = [];
				for (let k = 0; k < 400000; k++) junk.push({ k });
				junk = null;
				globalThis.gc();
				const text = readFileSync(${JSON.stringify(json_path)}, 'utf8');
				const t = performance.now();
				JSON.parse(text);
				console.log(performance.now() - t);
			`);
			parse_runs.push(Number(parse_out.trim()));
			const decode_out = await deps.run_node(
				`
				import { readFileSync } from 'node:fs';
				// a fresh V8 process's first young-generation collection costs milliseconds and lands in
				// whichever phase crosses the initial heap limit: pay it before the clock starts
				let junk = [];
				for (let k = 0; k < 400000; k++) junk.push({ k });
				junk = null;
				globalThis.gc();
				const tape = new Uint8Array(readFileSync(${JSON.stringify(tape_path)}));
				const source = readFileSync(${JSON.stringify(source_path)}, 'utf8');
				const t0 = performance.now();
				const { decode_tape } = await import(${JSON.stringify(decoder_url)});
				const t1 = performance.now();
				decode_tape(tape, source);
				console.log(` +
					'`' +
					`\${t1 - t0} \${performance.now() - t1}` +
					'`' +
					`);
			`
			);
			const [load, first] = decode_out.trim().split(' ').map(Number);
			import_runs.push(load!);
			decode_runs.push(first!);
			total_runs.push(load! + first!);
		}
		console.log(
			`  ${language} (${doc.path}, ${(doc.json.length / 1e3).toFixed(0)} kB JSON): JSON.parse ${median(parse_runs).toFixed(2)} ms; decoder load ${median(import_runs).toFixed(2)} ms + first decode_tape ${median(decode_runs).toFixed(2)} ms (together ${median(total_runs).toFixed(2)} ms)`
		);
	}
	void sink;
	return 0;
}
