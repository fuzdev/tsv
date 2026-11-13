/**
 * `run_wasm_format_probe` and `run_wasm_memory_probe` over fake glue builds and a fake corpus:
 * the byte-identity gate a `--baseline` must pass, the memory report each mode builds from the
 * instance's linear memory, a load that yields no memory, and a build that throws or crashes on a
 * source the other formats. Package-free, so it gates in
 * `npm run test:node`.
 */

import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';
import { afterEach, beforeEach, test, vi } from 'vitest';

import type { Language, SourceFile } from '../lib/types.ts';
import { raise_wasm_trap } from '../lib/wasm_trap_fakes.ts';
import {
	compare_format_outcomes,
	FormatThrew,
	run_wasm_format_probe,
	type WasmGlueExports
} from './wasm_format_probe.ts';
import { type FreshGlue, run_wasm_memory_probe } from './wasm_memory_probe.ts';

let err: string[];
beforeEach(() => {
	err = [];
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
});
afterEach(() => {
	vi.restoreAllMocks();
});

const FILES: SourceFile[] = (['a.css', 'b.svelte', 'c.ts'] as const).map(
	(path, i) =>
		({
			path,
			language: (['css', 'svelte', 'typescript'] as Language[])[i],
			content: 'x'.repeat(i + 1),
			bytes: i + 1
		}) as SourceFile
);

/** A glue build whose formatters append `suffix`, growing `memory` by a page per call. */
const glue = (suffix: string, memory?: WebAssembly.Memory): WasmGlueExports => {
	const format = (source: string) => {
		memory?.grow(1);
		return source + suffix;
	};
	return { format_css: format, format_svelte: format, format_typescript: format };
};

/**
 * A glue build that formats as `glue(suffix)` does, but refuses each source holding a char of
 * `refuses` the way the wasm glue does: a fresh `{ok: false, error}` object per call.
 */
const refusing_glue = (
	suffix: string,
	refuses: string,
	memory?: WebAssembly.Memory
): WasmGlueExports => {
	const format = (source: string) => {
		memory?.grow(1);
		if ([...refuses].some((c) => source.includes(c))) {
			return { ok: false, error: { kind: 'parse', message: 'refused', frame: '' } };
		}
		return source + suffix;
	};
	return { format_css: format, format_svelte: format, format_typescript: format };
};

/** `FILES`, with a fourth the refusing builds below refuse. */
const FILES_WITH_REFUSED: SourceFile[] = [
	...FILES,
	{ path: 'd.css', language: 'css', content: 'r', bytes: 1 } as SourceFile
];

test('outcomes compare by their bytes: equal text across calls, differing text, a refusal on one side', () => {
	strictEqual(compare_format_outcomes(['a {}', 'a {}'.slice(0)]), 'identical');
	strictEqual(compare_format_outcomes(['a {}', 'a{}']), 'mismatch');
	strictEqual(compare_format_outcomes(['a {}', { ok: false }]), 'mismatch');
	strictEqual(compare_format_outcomes([{ ok: false }, 'a {}']), 'mismatch');
	strictEqual(compare_format_outcomes([{ ok: false }, { ok: false }]), 'skipped');
	strictEqual(compare_format_outcomes(['a {}', 'b {}']), 'mismatch');
	strictEqual(compare_format_outcomes(['a {}']), 'identical');
});

/** What a build's call threw, as `format_outcome` holds it. */
const threw = (make: () => never): FormatThrew => {
	try {
		make();
	} catch (e) {
		return new FormatThrew(e);
	}
};
const TYPE_ERROR = threw(() => {
	throw new TypeError('expected a string');
});
const OTHER_TYPE_ERROR = threw(() => {
	throw new TypeError('expected a language');
});
const TRAP = threw(() => raise_wasm_trap());

test('a throw on one side only is a mismatch, the same throw on every side a skip, a crash a crash', () => {
	strictEqual(compare_format_outcomes([TYPE_ERROR, TYPE_ERROR]), 'skipped');
	strictEqual(compare_format_outcomes(['a {}', TYPE_ERROR]), 'mismatch');
	strictEqual(compare_format_outcomes([TYPE_ERROR, 'a {}']), 'mismatch');
	strictEqual(compare_format_outcomes([{ ok: false }, TYPE_ERROR]), 'mismatch');
	strictEqual(compare_format_outcomes([TYPE_ERROR, OTHER_TYPE_ERROR]), 'mismatch');
	strictEqual(compare_format_outcomes(['a {}', TRAP]), 'crash');
	strictEqual(compare_format_outcomes([TRAP, { ok: false }]), 'crash');
	strictEqual(compare_format_outcomes([TRAP, TRAP]), 'crash');
});

/** A glue build that formats as `glue('')` does, but throws `make()` on a source holding `r`. */
const throwing_glue = (make: () => never, memory?: WebAssembly.Memory): WasmGlueExports => {
	const format = (source: string) => {
		memory?.grow(1);
		if (source.includes('r')) make();
		return source;
	};
	return { format_css: format, format_svelte: format, format_typescript: format };
};
const type_error = (): never => {
	throw new TypeError('expected a string');
};

test('the format probe skips a source both builds refuse, and passes an A/B that agrees', async () => {
	const code = await run_wasm_format_probe(
		{ load_glue: async () => refusing_glue('', 'r'), load_files: async () => FILES_WITH_REFUSED },
		['corpus', '--baseline', 'base.js', '--pairs', '1', '--warmup', '0']
	);
	strictEqual(code, 0);
	ok(err.includes('\nbyte-identity: 3/3 identical'), err.join('\n'));
	ok(err.some((l) => l.startsWith('Probing 3 files')));
});

test('the format probe aborts an A/B where one build refuses a source the other formats', async () => {
	const code = await run_wasm_format_probe(
		{
			load_glue: async (path) => refusing_glue('', path === 'base.js' ? 'r' : ''),
			load_files: async () => FILES_WITH_REFUSED
		},
		['corpus', '--baseline', 'base.js', '--pairs', '1', '--warmup', '0']
	);
	strictEqual(code, 1);
	ok(err.includes('\nbyte-identity: 3/4 identical  — 1 MISMATCH'), err.join('\n'));
});

test('the format probe aborts when a baseline formats differently', async () => {
	const loaded: string[] = [];
	const code = await run_wasm_format_probe(
		{
			load_glue: async (path) => {
				loaded.push(path);
				return glue(path === 'base.js' ? '!' : '');
			},
			load_files: async () => FILES
		},
		['corpus', '--baseline', 'base.js', '--pairs', '1', '--warmup', '0']
	);
	strictEqual(code, 1);
	deepStrictEqual(loaded, ['crates/tsv_wasm/pkg/all/nodejs/tsv_wasm.js', 'base.js']);
	ok(err.includes('\nbyte-identity: 0/3 identical  — 3 MISMATCH'));
});

test('the format probe times identical builds and reports each language', async () => {
	const code = await run_wasm_format_probe(
		{ load_glue: async () => glue(''), load_files: async () => FILES },
		['corpus', '--baseline', 'base.js', '--pairs', '2', '--warmup', '0']
	);
	strictEqual(code, 0);
	ok(err.includes('\nbyte-identity: 3/3 identical'));
	for (const label of ['svelte', 'typescript', 'css', 'total']) {
		ok(err.some((l) => l.trimStart().startsWith(`${label} `) && l.includes(' A/B ')));
	}
});

test('the memory probe reports each cold format’s reserve and growth from a fresh memory', async () => {
	let out = '';
	let loads = 0;
	const code = await run_wasm_memory_probe(
		{
			load_fresh_glue: async (): Promise<FreshGlue> => {
				loads++;
				const memory = new WebAssembly.Memory({ initial: 2 });
				return { exports: glue('', memory), memory };
			},
			load_files: async () => FILES,
			stdout: { write: async (data) => void (out += data) }
		},
		['corpus', '--cold', '--json']
	);
	strictEqual(code, 0);
	strictEqual(loads, 3);
	const current = JSON.parse(out).builds.current;
	strictEqual(current.reserve_bytes, 2 * 65536);
	strictEqual(current.peak_max, 3 * 65536);
	strictEqual(current.growth_max, 65536);
	strictEqual(current.n, 3);
});

test('the memory probe’s steady mode reuses one instance across the corpus', async () => {
	let out = '';
	const code = await run_wasm_memory_probe(
		{
			load_fresh_glue: async () => {
				const memory = new WebAssembly.Memory({ initial: 1 });
				return { exports: glue('', memory), memory };
			},
			load_files: async () => FILES,
			stdout: { write: async (data) => void (out += data) }
		},
		['corpus', '--json']
	);
	strictEqual(code, 0);
	deepStrictEqual(JSON.parse(out).builds.current, {
		reserve_bytes: 65536,
		high_water_bytes: 4 * 65536,
		total_growth_bytes: 3 * 65536,
		processed: 3
	});
});

test('the memory probe aborts an A/B whose builds format differently', async () => {
	const code = await run_wasm_memory_probe(
		{
			load_fresh_glue: async (path) => {
				const memory = new WebAssembly.Memory({ initial: 1 });
				return { exports: glue(path === 'base.js' ? '!' : '', memory), memory };
			},
			load_files: async () => FILES,
			stdout: { write: async () => {} }
		},
		['corpus', '--baseline', 'base.js']
	);
	strictEqual(code, 1);
	ok(err.some((l) => l.startsWith('ABORT: builds format differently on 3/3 files')));
});

test('the memory probe passes an A/B whose builds refuse the same source', async () => {
	const code = await run_wasm_memory_probe(
		{
			load_fresh_glue: async () => {
				const memory = new WebAssembly.Memory({ initial: 1 });
				return { exports: refusing_glue('', 'r', memory), memory };
			},
			load_files: async () => FILES_WITH_REFUSED,
			stdout: { write: async () => {} }
		},
		['corpus', '--baseline', 'base.js']
	);
	strictEqual(code, 0);
	ok(err.includes('byte-identity: 4/4 identical'), err.join('\n'));
});

test('a glue load that yields no memory is refused', async () => {
	await rejects(
		run_wasm_memory_probe(
			{
				load_fresh_glue: async () => ({ exports: glue(''), memory: null }),
				load_files: async () => FILES,
				stdout: { write: async () => {} }
			},
			['corpus', '--baseline', 'base.js']
		),
		/no memory captured for crates\/tsv_wasm\/pkg\/all\/nodejs\/tsv_wasm\.js/
	);
});

test('the format probe aborts an A/B where one build throws on a source the other formats', async () => {
	const code = await run_wasm_format_probe(
		{
			load_glue: async (path) => (path === 'base.js' ? glue('') : throwing_glue(type_error)),
			load_files: async () => FILES_WITH_REFUSED
		},
		['corpus', '--baseline', 'base.js', '--pairs', '1', '--warmup', '0']
	);
	strictEqual(code, 1);
	ok(err.includes('\nbyte-identity: 3/4 identical  — 1 MISMATCH'), err.join('\n'));
	ok(
		err.includes('  MISMATCH d.css: current threw (expected a string), baseline formatted'),
		err.join('\n')
	);
});

test('the format probe skips a source every build throws the same error on', async () => {
	const code = await run_wasm_format_probe(
		{
			load_glue: async () => throwing_glue(type_error),
			load_files: async () => FILES_WITH_REFUSED
		},
		['corpus', '--baseline', 'base.js', '--pairs', '1', '--warmup', '0']
	);
	strictEqual(code, 0);
	ok(err.includes('\nbyte-identity: 3/3 identical'), err.join('\n'));
});

test('the format probe fails on a build crashing, listed with the build and the file, A/A too', async () => {
	const code = await run_wasm_format_probe(
		{
			load_glue: async () => throwing_glue(() => raise_wasm_trap()),
			load_files: async () => FILES_WITH_REFUSED
		},
		['corpus', '--pairs', '1', '--warmup', '0']
	);
	strictEqual(code, 1);
	const at = err.findIndex((l) => l.startsWith('FAIL: 1 panic(s)'));
	ok(at > 0, err.join('\n'));
	deepStrictEqual(err.slice(at + 1), ['  current: d.css: RuntimeError: unreachable']);
});

test('the memory probe aborts an A/B where one build throws on a source the other formats', async () => {
	const code = await run_wasm_memory_probe(
		{
			load_fresh_glue: async (path) => {
				const memory = new WebAssembly.Memory({ initial: 1 });
				const exports = path === 'base.js' ? glue('', memory) : throwing_glue(type_error, memory);
				return { exports, memory };
			},
			load_files: async () => FILES_WITH_REFUSED,
			stdout: { write: async () => {} }
		},
		['corpus', '--baseline', 'base.js']
	);
	strictEqual(code, 1);
	ok(
		err.includes('  MISMATCH d.css: current threw (expected a string), baseline formatted'),
		err.join('\n')
	);
	ok(err.some((l) => l.startsWith('ABORT: builds format differently on 1/4 files')));
});

test('the memory probe fails an A/B where a build crashes, listed with the build and the file', async () => {
	const code = await run_wasm_memory_probe(
		{
			load_fresh_glue: async (path) => {
				const memory = new WebAssembly.Memory({ initial: 1 });
				const exports =
					path === 'base.js' ? throwing_glue(() => raise_wasm_trap(), memory) : glue('', memory);
				return { exports, memory };
			},
			load_files: async () => FILES_WITH_REFUSED,
			stdout: { write: async () => {} }
		},
		['corpus', '--baseline', 'base.js']
	);
	strictEqual(code, 1);
	const at = err.findIndex((l) => l.startsWith('FAIL: 1 panic(s)'));
	ok(at >= 0, err.join('\n'));
	deepStrictEqual(err.slice(at + 1), ['  baseline: d.css: RuntimeError: unreachable']);
	// the byte-identity gate fails it, before any measurement
	ok(!err.some((l) => l.includes('WASM memory probe')), err.join('\n'));
});

test('the memory probe fails a measurement that crashed, after its report, in either mode', async () => {
	for (const mode of [['--cold'], []]) {
		err = [];
		let out = '';
		const code = await run_wasm_memory_probe(
			{
				load_fresh_glue: async () => {
					const memory = new WebAssembly.Memory({ initial: 1 });
					return { exports: throwing_glue(() => raise_wasm_trap(), memory), memory };
				},
				load_files: async () => FILES_WITH_REFUSED,
				stdout: { write: async (data) => void (out += data) }
			},
			['corpus', '--json', ...mode]
		);
		strictEqual(code, 1, mode.join());
		ok(JSON.parse(out).builds.current, mode.join());
		const at = err.findIndex((l) => l.startsWith('FAIL: 1 panic(s)'));
		ok(at >= 0, err.join('\n'));
		deepStrictEqual(err.slice(at + 1), ['  current: d.css: RuntimeError: unreachable']);
	}
});

test('the memory probe skips a measurement whose build threw no crash', async () => {
	const code = await run_wasm_memory_probe(
		{
			load_fresh_glue: async () => {
				const memory = new WebAssembly.Memory({ initial: 1 });
				return { exports: throwing_glue(type_error, memory), memory };
			},
			load_files: async () => FILES_WITH_REFUSED,
			stdout: { write: async () => {} }
		},
		['corpus', '--json']
	);
	strictEqual(code, 0);
	ok(!err.some((l) => l.includes('panic(s)')));
});
