/**
 * Smoke test for all formatter and parser implementations.
 *
 * Catches "totally broken" implementations (throws, returns empty/null, not
 * idempotent) on trivial fixed inputs. Not a correctness gate — corpus_compare_format
 * is the gate. This is a fast sanity check that the benchmark harness has
 * something real to measure.
 *
 * Run: deno task smoke
 * Exit codes: 0 = all pass, 1 = any failure.
 */

import { exit } from 'node:process';
import { check_executed_artifacts } from './lib/check_artifact_freshness.ts';
import { check_node_modules } from './lib/check_node_modules.ts';
import {
	type BenchmarkTask,
	get_benchmark_tasks,
	init_implementations
} from './lib/implementations.ts';
import { current_runtime } from './lib/runtime.ts';
import { type Language, LANGUAGES } from './lib/types.ts';

/**
 * Trivial unformatted inputs per language. Each touches a small mix of
 * structural elements (object literals, types, selectors, comments,
 * at-rules) so a formatter that only handles the most degenerate shape
 * still trips. Kept syntactically valid across every parser we run.
 */
const INPUTS: Record<Language, string> = {
	svelte:
		'<script lang="ts">const x={a:1,b:2};/* c */function f(n:number){return n*2}</script>\n<div   class="a b"   >{x.a}</div>\n<style>.a{color:red;display:flex}.a:hover{gap:1px}</style>',
	typescript:
		'import {x} from "y";const o={a:1,b:2};function f<T>(n:T):T{return n}/* c */type U={a:number;b:string};async function g(){await Promise.resolve(1)}',
	css: '/* c */@media (min-width:1px){.foo>.bar:hover{color:red;display:flex;gap:1px}}.baz,.qux{margin:0}'
};

interface Failure {
	kind: 'format' | 'parse';
	lang: Language;
	impl: string;
	reason: string;
}

const failures: Failure[] = [];
let passed = 0;

function record_pass(): void {
	passed++;
}

function record_fail(f: Failure): void {
	failures.push(f);
}

// Refuse to smoke stale binaries (smoke skips the rebuild for speed, same as the
// bench/corpus :run tasks). Which artifacts this runtime executes — FFI or N-API,
// plus that runtime's WASM target — is `check_executed_artifacts`'s subject;
// override with BENCH_STALE_OK=1.
await check_executed_artifacts();

// Friendly preflight: the canonical impls (prettier + svelte/compiler) resolve
// from the harness node_modules; without it, init fails opaquely. Missing or
// stale is fatal — see lib/check_node_modules.ts (BENCH_STALE_OK=1 for stale).
await check_node_modules();

const impls = await init_implementations({ logger: () => {} });

// The init chatter is silenced above (the per-impl ✓ lines would drown the check
// table), so the ⚠ lines go with it — which would leave an impl that stopped
// LOADING invisible here. That is the one failure this file exists to catch and the
// one its check table structurally cannot show: a task list built from `impls` has
// no row for an impl that isn't there, so the run reports a smaller `passed` count
// against no baseline and calls itself green. Name them instead.
if (impls.unavailable.length > 0) {
	console.log(`Unavailable (${impls.unavailable.length}) — no rows to check:`);
	for (const u of impls.unavailable) {
		console.log(`  ⚠ ${u.impl.padEnd(20)} ${u.reason.slice(0, 90)}`);
	}
	console.log();
}

/**
 * Every task for one operation+language, across BOTH corpus surfaces.
 *
 * `get_benchmark_tasks` takes a `corpus_kind` because two rows are
 * surface-specific in opposite directions (yuku's N-API row is perf-only, `tsc` is
 * conformance-only — see its `BenchmarkTaskOptions`), and a single-surface call
 * here would leave whichever row the other surface owns un-smoked. That is the gap
 * this file's one-registry rule exists to prevent, so take the union and dedupe by
 * tracking key. Smoke's input is trivial and valid, so a row excluded from a corpus
 * for that corpus's reasons is still meaningful to sanity-check here.
 */
function all_surface_tasks(operation: 'parse' | 'format', language: Language): BenchmarkTask[] {
	const tasks: BenchmarkTask[] = [];
	const seen = new Set<string>();
	for (const corpus_kind of ['perf', 'conformance'] as const) {
		for (const task of get_benchmark_tasks(impls, operation, language, { corpus_kind })) {
			if (seen.has(task.tracking_key)) continue;
			seen.add(task.tracking_key);
			tasks.push(task);
		}
	}
	return tasks;
}

//
// Formatters
//

console.log('Formatters:');

// One registry for both halves of this file and for the bench: `get_benchmark_tasks`.
// A second formatter list here would let a newly added impl reach the bench while
// silently missing from smoke. Its per-language list simply omits an impl that
// declines the language, so the union across languages is what still lets the
// `(unsupported)` line name it — the signal that an impl LOADED but doesn't do this
// language, as opposed to not loading at all.
const format_tasks_by_language = new Map(
	LANGUAGES.map((lang) => [lang, all_surface_tasks('format', lang)] as const)
);
const format_names = [
	...new Set([...format_tasks_by_language.values()].flat().map((task) => task.name))
];

for (const lang of LANGUAGES) {
	console.log(`  ${lang}:`);
	const input = INPUTS[lang];
	const by_name = new Map(format_tasks_by_language.get(lang)!.map((task) => [task.name, task]));

	for (const name of format_names) {
		const task = by_name.get(name);
		if (!task) {
			console.log(`    ${name.padEnd(12)} - (unsupported)`);
			continue;
		}

		const call = (src: string): Promise<unknown> =>
			task.is_async ? task.run_async!(src, lang) : Promise.resolve(task.run(src, lang));

		let first: unknown;
		try {
			first = await call(input);
		} catch (e) {
			const msg = e instanceof Error ? e.message : String(e);
			console.log(`    ${name.padEnd(12)} ✗ threw: ${msg.slice(0, 80)}`);
			record_fail({ kind: 'format', lang, impl: name, reason: `threw: ${msg}` });
			continue;
		}

		if (typeof first !== 'string' || first.length === 0) {
			console.log(`    ${name.padEnd(12)} ✗ empty or non-string output`);
			record_fail({ kind: 'format', lang, impl: name, reason: 'empty/non-string output' });
			continue;
		}

		let second: unknown;
		try {
			second = await call(first);
		} catch (e) {
			const msg = e instanceof Error ? e.message : String(e);
			console.log(`    ${name.padEnd(12)} ✗ second pass threw: ${msg.slice(0, 80)}`);
			record_fail({
				kind: 'format',
				lang,
				impl: name,
				reason: `second pass threw: ${msg}`
			});
			continue;
		}

		if (first !== second) {
			console.log(`    ${name.padEnd(12)} ✗ not idempotent`);
			console.log(`      first:  ${JSON.stringify(first)}`);
			console.log(`      second: ${JSON.stringify(second)}`);
			record_fail({ kind: 'format', lang, impl: name, reason: 'not idempotent' });
			continue;
		}

		console.log(`    ${name.padEnd(12)} ✓`);
		record_pass();
	}
}

//
// Parsers
//

console.log('\nParsers:');
for (const lang of LANGUAGES) {
	console.log(`  ${lang}:`);
	const input = INPUTS[lang];
	const tasks = all_surface_tasks('parse', lang);

	for (const task of tasks) {
		try {
			const result = task.is_async ? await task.run_async!(input, lang) : task.run(input, lang);
			// Internal parsers return void; treat that as success.
			if (task.name.includes('internal')) {
				console.log(`    ${task.name.padEnd(20)} ✓`);
				record_pass();
				continue;
			}
			if (result == null) {
				console.log(`    ${task.name.padEnd(20)} ✗ null result`);
				record_fail({ kind: 'parse', lang, impl: task.name, reason: 'null result' });
				continue;
			}
			console.log(`    ${task.name.padEnd(20)} ✓`);
			record_pass();
		} catch (e) {
			const msg = e instanceof Error ? e.message : String(e);
			console.log(`    ${task.name.padEnd(20)} ✗ threw: ${msg.slice(0, 80)}`);
			record_fail({ kind: 'parse', lang, impl: task.name, reason: `threw: ${msg}` });
		}
	}
}

//
// Summary
//

console.log();
// A count with no denominator reads as green at any size, so qualify it whenever
// impls are missing — `All 42 checks passed` on a machine that should run 46 is the
// exact sentence this file must not say without saying why.
const shortfall =
	impls.unavailable.length > 0
		? ` (${impls.unavailable.length} impl(s) unavailable: ${impls.unavailable.map((u) => u.impl).join(', ')})`
		: '';
if (failures.length === 0) {
	console.log(`All ${passed} checks passed.${shortfall}`);
	exit(0);
} else {
	console.log(`${failures.length} failure(s), ${passed} passed:`);
	for (const f of failures) {
		console.log(`  ${f.kind}/${f.lang}/${f.impl}: ${f.reason}`);
	}
	exit(1);
}
