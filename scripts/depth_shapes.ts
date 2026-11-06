/**
 * The depth limits' shapes as the package-level depth gates read them: the spec lines
 * `tests/depth_limit.rs`'s `write_shape_limits` writes (every shape the native gate
 * generates, at its accepted depth), the source a spec makes at a depth, the depth
 * refusal's messages, and the calls a shape is graded through — the same for every engine
 * that ships the `(source, options?)` facade. Shared by `scripts/depth_wasm.ts` (the WASM
 * package on a Node main thread) and `scripts/pgo/addon_depth.ts` (the N-API addon on a
 * sized worker thread).
 *
 * Node builtins only.
 *
 * @module
 */

import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import process from 'node:process';

/** One shape at its accepted depth, as `write_shape_limits` writes it. */
export interface ShapeLimit {
	name: string;
	language: 'typescript' | 'css' | 'svelte';
	n: number;
	pre: string;
	unit: string;
	mid: string;
	close: string;
	post: string;
}

const unescape = (s: string): string =>
	s.replace(/\\(.)/g, (_, c: string) => (c === 'n' ? '\n' : c === 't' ? '\t' : c));

/** Every spec line of a `write_shape_limits` file. */
export function read_shapes(path: string): Array<ShapeLimit> {
	return readFileSync(path, 'utf8')
		.split('\n')
		.filter((line) => line.length > 0)
		.map((line) => {
			const [name, language, n, pre, unit, mid, close, post] = line.split('\t');
			return {
				name: unescape(name!),
				language: language as ShapeLimit['language'],
				n: Number(n),
				pre: unescape(pre!),
				unit: unescape(unit!),
				mid: unescape(mid!),
				close: unescape(close!),
				post: unescape(post!)
			};
		});
}

/** The source a shape makes at depth `n`. */
export const make_shape = (shape: ShapeLimit, n: number): string =>
	shape.pre + shape.unit.repeat(n) + shape.mid + shape.close.repeat(n) + shape.post;

/** The depth refusal's two messages (`tsv_lang::DepthLimit`). */
export const is_depth_refusal = (message: string): boolean =>
	message.includes('depth limit of') || message.includes('length limit');

/**
 * The calls a shape is graded through at its depth, each `[phase, call]`, in order: the
 * parse (the object form, with and without `{locations: true}`, and the `_json` string),
 * the format, the format OUTPUT parsed again and formatted to itself (F1 — the output nests
 * deeper in frames than the input, since the printer adds the pairs the input's parse only
 * charged, as the native gate's reparse of it grades), the import records (TypeScript and
 * Svelte) and the type strip (TypeScript). The output phase reads the format phase's result,
 * so the calls run in this order.
 */
export const shape_calls = (
	// deno-lint-ignore no-explicit-any
	tsv: any,
	shape: ShapeLimit,
	source: string
): Array<[string, () => unknown]> => {
	const lang = shape.language;
	// deno-lint-ignore no-explicit-any
	let formatted: any;
	return [
		['parse', () => tsv[`parse_${lang}`](source)],
		['parse+locations', () => tsv[`parse_${lang}`](source, { locations: true })],
		['parse_json', () => tsv[`parse_${lang}_json`](source)],
		['format', () => (formatted = tsv[`format_${lang}`](source))],
		[
			'format output',
			() => {
				if (formatted?.ok !== true) {
					return { ok: false, error: { message: 'the format returned no output to reparse' } };
				}
				const output: string = formatted.code;
				const reparsed = tsv[`parse_${lang}_json`](output);
				if (reparsed?.ok !== true) return reparsed;
				const again = tsv[`format_${lang}`](output);
				if (again?.ok !== true) return again;
				return again.code === output
					? { ok: true }
					: { ok: false, error: { message: 'the format output is not its own format (F1)' } };
			}
		],
		...(lang === 'css'
			? []
			: [['imports', () => tsv[`parse_imports_${lang}`](source)] as [string, () => unknown]]),
		...(lang === 'typescript'
			? [['strip', () => tsv.strip_typescript(source)] as [string, () => unknown]]
			: [])
	];
};

/** Whether a graded call's result passes: `{ok: true, …}`, or a strip's refusal (a verdict). */
export const call_passes = (phase: string, result: unknown): boolean => {
	// deno-lint-ignore no-explicit-any
	const r = result as any;
	return (
		r?.ok === true || (phase === 'strip' && r?.ok === false && r.error?.kind === 'unsupported')
	);
};

/**
 * Write every shape's spec line at its accepted depth to `path`, from the native test
 * (`cargo test -p tsv_tests --test depth_limit -- --ignored write_shape_limits`). The
 * limits are counts, not stack measurements, so any optimized profile writes the same
 * lines; `profile` and `features` pick the build to share (a caller passes what the
 * sibling task it follows built, so nothing compiles twice). Returns whether it succeeded.
 */
export function write_shape_limits(
	path: string,
	options: { cwd: string; profile: string; features?: string[]; env?: Record<string, string> }
): boolean {
	console.log(
		`writing the shapes and their limits (cargo test --profile ${options.profile} … write_shape_limits)`
	);
	const result = spawnSync(
		'cargo',
		[
			'test',
			'--locked',
			'--profile',
			options.profile,
			'-p',
			'tsv_tests',
			...(options.features?.length ? ['--features', options.features.join(',')] : []),
			'--test',
			'depth_limit',
			'--quiet',
			'--',
			'--ignored',
			'--exact',
			'write_shape_limits'
		],
		{
			cwd: options.cwd,
			stdio: 'inherit',
			env: { ...process.env, ...options.env, TSV_DEPTH_SHAPES: path }
		}
	);
	return result.status === 0;
}
