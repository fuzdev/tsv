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

import {
	get_benchmark_tasks,
	get_formatters,
	init_implementations,
} from './lib/implementations.ts';
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
	css:
		'/* c */@media (min-width:1px){.foo>.bar:hover{color:red;display:flex;gap:1px}}.baz,.qux{margin:0}',
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

const impls = await init_implementations({ logger: () => {} });

//
// Formatters
//

console.log('Formatters:');
const formatters = get_formatters(impls);

for (const lang of LANGUAGES) {
	console.log(`  ${lang}:`);
	const input = INPUTS[lang];

	for (const fmt of formatters) {
		if (!fmt.supports_language(lang)) {
			console.log(`    ${fmt.name.padEnd(12)} - (unsupported)`);
			continue;
		}

		const call = (src: string) =>
			fmt.is_async ? fmt.format_async!(src, lang) : Promise.resolve(fmt.format!(src, lang));

		let first: string;
		try {
			first = await call(input);
		} catch (e) {
			const msg = e instanceof Error ? e.message : String(e);
			console.log(`    ${fmt.name.padEnd(12)} ✗ threw: ${msg.slice(0, 80)}`);
			record_fail({ kind: 'format', lang, impl: fmt.name, reason: `threw: ${msg}` });
			continue;
		}

		if (typeof first !== 'string' || first.length === 0) {
			console.log(`    ${fmt.name.padEnd(12)} ✗ empty or non-string output`);
			record_fail({ kind: 'format', lang, impl: fmt.name, reason: 'empty/non-string output' });
			continue;
		}

		let second: string;
		try {
			second = await call(first);
		} catch (e) {
			const msg = e instanceof Error ? e.message : String(e);
			console.log(`    ${fmt.name.padEnd(12)} ✗ second pass threw: ${msg.slice(0, 80)}`);
			record_fail({
				kind: 'format',
				lang,
				impl: fmt.name,
				reason: `second pass threw: ${msg}`,
			});
			continue;
		}

		if (first !== second) {
			console.log(`    ${fmt.name.padEnd(12)} ✗ not idempotent`);
			console.log(`      first:  ${JSON.stringify(first)}`);
			console.log(`      second: ${JSON.stringify(second)}`);
			record_fail({ kind: 'format', lang, impl: fmt.name, reason: 'not idempotent' });
			continue;
		}

		console.log(`    ${fmt.name.padEnd(12)} ✓`);
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
	const tasks = get_benchmark_tasks(impls, 'parse', lang);

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
if (failures.length === 0) {
	console.log(`All ${passed} checks passed.`);
	Deno.exit(0);
} else {
	console.log(`${failures.length} failure(s), ${passed} passed:`);
	for (const f of failures) {
		console.log(`  ${f.kind}/${f.lang}/${f.impl}: ${f.reason}`);
	}
	Deno.exit(1);
}
