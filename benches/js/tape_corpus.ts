/**
 * The tape corpus leg — `npm run tape:corpus`, run by `npm run conformance`: `check:tape`'s grades
 * over real code instead of the fixtures.
 *
 * **Corpus.** Every file of the `gates` view's snapshot tiers — the `../corpora` collections,
 * pinned by tree id — in the three languages: the JS/TS family, `.svelte`, `.css`. A TypeScript
 * file parses at the module goal, as the bindings' parse does; one tsv rejects there but parses
 * at the script goal (a sloppy script) has no wire at the parse's goal and is counted, not
 * graded — its verdict is `corpus:compare:parse`'s. Any other error fails the file: a Svelte or
 * CSS reject, a TypeScript file that parses at neither goal.
 *
 * **Per file**, from one `tsv_debug tape --stdin` process (the `--profile corpus` build, so a
 * panic is caught and answered as one): the span-only wire and the transcoder's tape. The file
 * fails on a panic, on the transcoder refusing the wire, on the offline encoder's tape
 * (`scripts/tape_encode.ts`) differing from the transcoder's by a byte, or on the shipped decoder
 * — its fast path, or its iterative interpreter alone — reading back a tree that differs from
 * `JSON.parse`'s of the wire, key order included (`scripts/check_tape.ts`'s `grade_document`), and
 * each decode in loc mode reading back the same parse's `loc`-bearing wire.
 * Every file's direct tape — its wire writer's own — is graded too: the Rust side holds it
 * to the wire (printback, and the transcoder as canonicalizer — a failure is the reply's
 * `direct:` error), and both decoder paths must read it back to the same tree
 * (`grade_direct_tape`).
 * The run fails on an empty corpus, and on a run that graded no file.
 *
 * **The hot-shape list.** The author's own collections (manifest `origin: own`, held off the
 * perf bench) are also counted, per language, by the fast shape each record takes — the census
 * the committed hot-shape list (`tsv_tape_hot.json`, `hot_file_from_counts`) is cut from, so
 * the shapes the decoder's loc mode twins are chosen on code the bench never times. A run
 * whose census moves a shape across a language's cut fails (a rank change inside a cut does not:
 * the decoder reads the list as a set); `--update-hot`
 * (`npm run tape-hot:update`) writes the census's list instead of grading, for
 * `npm run ast-schema:update` to regenerate the decoder from.
 *
 * Takes no arguments but `--update-hot`; to look at one file, ask `tsv_debug tape --stdin` for
 * it.
 *
 * Runs under Node (`benches/js/roots/tape_corpus.ts`, which binds the program): its reach is the
 * corpus, read through the corpus loader, the checkout's schema files, and the checkout's
 * `target/corpus/tsv_debug` (`TapeCorpusDeps`). It writes nothing but, under `--update-hot`,
 * the hot-shape list.
 *
 * @module
 */

import { readFile } from 'node:fs/promises';
import { relative, resolve } from 'node:path';

import { parse_ast_schema } from '../../scripts/ast_schema.ts';
import { grade_direct_tape, grade_document, wire_source } from '../../scripts/check_tape.ts';
import { encode_tape } from '../../scripts/tape_encode.ts';
import {
	build_tape_schema,
	hot_file_from_counts,
	layout_tape_hot,
	parse_tape_hot,
	parse_tape_shapes,
	shape_key,
	tape_hot_cut_changes,
	TAPE_HOT_PATH,
	TAPE_SHAPES_PATH
} from '../../scripts/tape_schema.ts';
import { CORPORA_COLLECTIONS, is_collection_path } from './lib/corpora.ts';
import { CorpusLoader, corpus_file_origin_resolver } from './lib/corpus.ts';
import { type TsvDebugProgram, TsvDebugServer } from './lib/tsv_debug_server.ts';

/** What `run_tape_corpus` reaches beyond the corpus loader's reads. */
export interface TapeCorpusDeps {
	/** Makes the checkout's `target/corpus/tsv_debug`, called once its freshness guard passed. */
	open_tsv_debug: () => TsvDebugProgram;
	/** Writes the hot-shape list's text (`TAPE_HOT_PATH`), under `--update-hot` alone. */
	write_hot_list: (text: string) => Promise<void>;
}

/** One `tsv_debug tape --stdin` request. */
export interface TapeRequest {
	language: 'svelte' | 'typescript' | 'css';
	source: string;
	goal?: 'script' | 'module';
	repeat?: number;
}

/** One reply line of `tsv_debug tape --stdin`. */
export type TapeReply =
	| { span: unknown; loc: unknown; tape: string; direct?: string; transcode_ns?: number }
	| { error: string }
	| { panic: string };

/** Failing files listed in full before the rest are only counted. */
const MAX_REPORTED = 20;

/**
 * The leg over the whole corpus, printing its report — or with `--update-hot`, the hot-shape
 * list's rewrite from the census alone.
 * @returns the exit code: `1` on any failure, `2` on an unknown argument, else `0`
 */
export async function run_tape_corpus(
	deps: TapeCorpusDeps,
	args: ReadonlyArray<string> = []
): Promise<number> {
	const update_hot = args.includes('--update-hot');
	const unknown = args.filter((arg) => arg !== '--update-hot');
	if (unknown.length > 0) {
		console.error(`unknown argument(s) ${unknown.join(' ')} — tape:corpus takes [--update-hot]`);
		return 2;
	}
	const table = parse_ast_schema(
		await readFile('crates/tsv_wasm/types/tsv_ast_schema.json', 'utf8')
	);
	const shapes = parse_tape_shapes(await readFile(TAPE_SHAPES_PATH, 'utf8'));
	const schema =
		'errors' in table ? table : 'errors' in shapes ? shapes : build_tape_schema(table, shapes);
	if ('errors' in schema) {
		console.error(`✗ the tape schema does not build:\n  ${schema.errors.join('\n  ')}`);
		return 1;
	}
	const server = await TsvDebugServer.start<TapeRequest, TapeReply>('tape', deps.open_tsv_debug);
	const started = performance.now();
	const collections_root = resolve(CORPORA_COLLECTIONS);
	const files = await new CorpusLoader('gates', { only: is_collection_path }).load(() => {});
	const origin_of = await corpus_file_origin_resolver();
	console.log(`tape corpus: ${files.length} file(s) from ${CORPORA_COLLECTIONS}`);
	// the hot-shape census: per language, records per fast shape over the `own` collections
	const census = {
		svelte: new Map<number, number>(),
		typescript: new Map<number, number>(),
		css: new Map<number, number>()
	};
	const failures: string[] = [];
	const graded = { svelte: 0, typescript: 0, css: 0 };
	let rejected = 0;
	let json_bytes = 0;
	let tape_bytes = 0;
	const direct = { svelte: 0, typescript: 0, css: 0 };
	for (const file of files) {
		const path = relative(collections_root, file.path);
		const source = file.content.toWellFormed();
		let reply: TapeReply;
		try {
			reply = await server.ask({ language: file.language, source });
		} catch (e) {
			console.error(`✗ ${e instanceof Error ? e.message : e}, on ${path}`);
			server.kill();
			return 1;
		}
		if ('panic' in reply) {
			failures.push(`${path}: tsv panicked: ${reply.panic}`);
			continue;
		}
		if ('error' in reply) {
			// only a TypeScript file tsv reads as a script, rejected at the parse's module goal,
			// has no wire to grade; any other error is a failure
			const script =
				file.language === 'typescript' && !reply.error.startsWith('transcode:')
					? await server.ask({ language: file.language, source, goal: 'script' })
					: null;
			if (script !== null && 'span' in script) rejected++;
			else failures.push(`${path}: ${reply.error.split('\n')[0]}`);
			continue;
		}
		const text = wire_source(file.language, source);
		if (origin_of(file) === 'own') {
			const counts = census[file.language];
			encode_tape(reply.span, text, schema, {
				census: (entry, mask) => {
					const id = schema.shape_ids.get(shape_key(entry, mask));
					if (id !== undefined) counts.set(id, (counts.get(id) ?? 0) + 1);
				}
			});
		}
		if (update_hot) continue;
		graded[file.language]++;
		const tape = Buffer.from(reply.tape, 'base64');
		json_bytes += Buffer.byteLength(JSON.stringify(reply.span));
		tape_bytes += tape.length;
		const loc = { tree: reply.loc, language: file.language };
		const problems = grade_document(reply.span, text, tape, schema, loc);
		if (reply.direct !== undefined) {
			direct[file.language]++;
			problems.push(
				...grade_direct_tape(reply.span, text, Buffer.from(reply.direct, 'base64'), loc)
			);
		}
		if (problems.length > 0) failures.push(`${path}: ${problems.join('; ')}`);
	}
	try {
		await server.close();
	} catch (e) {
		failures.push(e instanceof Error ? e.message : String(e));
	}
	if (files.length === 0) failures.push('the corpus is empty — nothing was graded');
	const hot_file = hot_file_from_counts(schema, census);
	const hot = layout_tape_hot(hot_file);
	if ([...Object.values(census)].every((counts) => counts.size === 0)) {
		failures.push('no file of an `own` collection was counted — the hot-shape census is empty');
	} else if (update_hot) {
		if (failures.length === 0) {
			await deps.write_hot_list(hot);
			console.log(`${TAPE_HOT_PATH}: rewritten from the census of ${files.length} file(s)`);
			return 0;
		}
	} else {
		// Compared as per-language SETS: the decoder reads the list as one, so a rank swap
		// inside a cut changes nothing it generates, and only a shape crossing a cut fails.
		const committed = parse_tape_hot(await readFile(TAPE_HOT_PATH, 'utf8'));
		const changes =
			'errors' in committed
				? committed.errors.map((e) => `does not read: ${e}`)
				: tape_hot_cut_changes(committed, hot_file);
		if (changes.length > 0) {
			failures.push(
				`${TAPE_HOT_PATH} is not the list this corpus's census cuts — run \`npm run tape-hot:update\` (it regenerates the decoder):`,
				...changes.map((c) => `  ${c}`)
			);
		}
	}
	if (update_hot) {
		for (const f of failures) console.error(`  ${f}`);
		return 1;
	}
	const seconds = ((performance.now() - started) / 1000).toFixed(1);
	const total = graded.svelte + graded.typescript + graded.css;
	if (total === 0) failures.push('no file was graded');
	for (const language of ['svelte', 'typescript', 'css'] as const) {
		if (graded[language] > 0 && direct[language] !== graded[language])
			failures.push(
				`${graded[language]} ${language} file(s) graded but ${direct[language]} direct tape(s)`
			);
	}
	console.log(
		`${total} file(s) graded (${graded.svelte} svelte, ${graded.typescript} typescript, ${graded.css} css), ` +
			`${rejected} tsv rejects at the parse's goal, ${direct.svelte} svelte + ${direct.typescript} typescript + ${direct.css} css direct tapes; tapes ${((100 * tape_bytes) / Math.max(json_bytes, 1)).toFixed(0)}% of the JSON's bytes — ${seconds}s`
	);
	if (failures.length > 0) {
		console.error(`\n✗ ${failures.length} failure(s):`);
		for (const f of failures.slice(0, MAX_REPORTED)) console.error(`  ${f}`);
		if (failures.length > MAX_REPORTED)
			console.error(`  … and ${failures.length - MAX_REPORTED} more`);
		return 1;
	}
	console.log('✓ every corpus wire round-trips through the tape, and both producers agree');
	return 0;
}
