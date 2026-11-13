/**
 * 4-way formatter differential triage: tsv vs prettier(-typescript) vs
 * biome-wasm vs oxfmt. A hand-triage aid for mining the Biome/oxfmt corpus
 * output — NOT a gate.
 *
 * The corpus:compare:format tool only diffs tsv vs prettier; this adds biome +
 * oxfmt as third/fourth opinions so a tsv-vs-prettier divergence can be bucketed:
 *   - tsv alone (prettier == biome == oxfmt, all differ from tsv) → candidate bug
 *   - tsv + biome (or + oxfmt) agree vs prettier → candidate sanctioned divergence
 *
 * Prettier is routed through the TYPESCRIPT parser (filepath `snippet.ts`), never
 * babel. That routing is load-bearing when working from Biome's "Differences with
 * Prettier" doc: it reports prettier's behavior under prettier's BABEL parser, while
 * tsv's oracle is prettier-TYPESCRIPT (prettier-plugin-svelte routes `lang="ts"`
 * through it). The two often differ — several "Prettier omits X" claims there are
 * babel-only and prettier-typescript actually does X — so a listed difference can
 * turn out to be a plain tsv bug rather than a divergence. Confirm every such claim
 * against the typescript parser before deciding it is one.
 *
 * tsv's side is the N-API addon at the `corpus` profile (`npm run build:napi:corpus`), behind
 * its freshness guard, so a panic is printed as that formatter's error.
 *
 * Usage (from repo root):
 *   node benches/js/roots/biome_oxfmt_diff.ts --parser typescript --content '<code>'
 *   echo '<code>' | node benches/js/roots/biome_oxfmt_diff.ts --parser typescript --stdin
 *   node benches/js/roots/biome_oxfmt_diff.ts --parser typescript --file path/to/file.ts
 *   node benches/js/roots/biome_oxfmt_diff.ts --parser typescript --files a.ts b.ts   # batch verdicts, no bodies
 *
 * `--files` takes every path after it: the argument parser gives the flag its first path as a
 * value and the rest as positionals, so with `--files` each positional is one more file (and a
 * repeated `--files` adds its own).
 *
 * `run_biome_oxfmt_diff` is the library function; its root,
 * `benches/js/roots/biome_oxfmt_diff.ts`, binds its deps (`BiomeOxfmtDiffDeps`).
 *
 * @module
 */

import { args_parse, argv_parse } from '@fuzdev/fuz_util/args.ts';
import * as s from '@fuzdev/fuz_schema/schema.ts';

import type { InitCompareImplementations } from '../lib/compare_cli.ts';
import { type Language, LANGUAGE_EXTENSIONS } from '../lib/types.ts';

const Args = s.strip_object({
	_: s.with_default_fn(s.array(s.string), () => []),
	parser: s.with_default(s.one_of(['typescript', 'svelte', 'css']), 'typescript'),
	content: s.optional(s.string),
	file: s.optional(s.string),
	// one path (`--files a.ts`, the rest positionals), or one per repeated `--files`
	files: s.optional(s.union([s.string, s.array(s.string)])),
	stdin: s.with_default(s.boolean, false),
	quiet: s.with_default(s.boolean, false)
});

type Out = { text?: string; err?: string };

/** The third and fourth opinions, by the names the report prints. */
export type DiffFormatterName = 'biome' | 'oxfmt';

/** One of the third-party formatters, initialized. */
export interface DiffFormatter {
	supports_format_language(language: Language): boolean;
	format(source: string, language: Language): string | Promise<string>;
	dispose(): void;
}

/** What `run_biome_oxfmt_diff` reaches the world through. */
export interface BiomeOxfmtDiffDeps {
	/** tsv's native binding and prettier, initialized; `null` when either failed to load. */
	init_implementations: InitCompareImplementations;
	/** The formatter `name` (biome's wasm, oxfmt), initialized; throws when it can't load. */
	load_formatter(name: DiffFormatterName): Promise<DiffFormatter>;
	/** A file's text, by the path `--file` / `--files` spells. */
	read_file(path: string): Promise<string>;
	/** All of stdin, as text. */
	read_stdin(): Promise<string>;
	stdout: { write(data: string): Promise<unknown> };
}

/**
 * Formats each source with tsv, prettier, biome, and oxfmt, and prints the outputs or, for
 * a batch, one agreement verdict per source.
 *
 * @param deps - the engines, the sources' readers, stdout
 * @param argv - the tool's arguments: `--parser`, and one of `--content`, `--stdin`,
 * `--file`, `--files`; `--quiet` prints verdicts only
 * @returns the exit code: 1 on bad arguments, no source, or an engine that failed to load
 */
export async function run_biome_oxfmt_diff(
	deps: BiomeOxfmtDiffDeps,
	argv: ReadonlyArray<string>
): Promise<number> {
	const print = (text: string): Promise<unknown> => deps.stdout.write(text + '\n');
	const args = args_parse(argv_parse([...argv]), Args);
	if (!args.ok) {
		console.error(args.issues.map(s.format_issue).join('\n'));
		return 1;
	}
	const a = args.value;
	const lang = a.parser as Language;
	const ext = LANGUAGE_EXTENSIONS[lang];

	const engines = await deps.init_implementations();
	if (engines === null) return 1;
	const { canonical, native } = engines;
	/** The formatter `name`, or `null` (reported) when it can't load. */
	const load = async (name: DiffFormatterName): Promise<DiffFormatter | null> => {
		try {
			return await deps.load_formatter(name);
		} catch (e) {
			console.error(`${name} unavailable: ${e}`);
			return null;
		}
	};
	const biome = await load('biome');
	const oxfmt = await load('oxfmt');
	const dispose = (): void => {
		engines.dispose();
		biome?.dispose();
		oxfmt?.dispose();
	};

	async function format_all(src: string): Promise<Record<string, Out>> {
		const out: Record<string, Out> = {};
		try {
			out.tsv = { text: native.format(src, lang) };
		} catch (e) {
			out.tsv = { err: String(e instanceof Error ? e.message : e) };
		}
		try {
			// Force the typescript parser for TS via a .ts filepath (never babel).
			out.prettier = { text: await canonical.format_async(src, lang, `snippet${ext}`) };
		} catch (e) {
			out.prettier = { err: String(e instanceof Error ? e.message : e) };
		}
		for (const [name, formatter] of [
			['biome', biome],
			['oxfmt', oxfmt]
		] as const) {
			if (!formatter?.supports_format_language(lang)) continue;
			try {
				out[name] = { text: await formatter.format(src, lang) };
			} catch (e) {
				out[name] = { err: String(e instanceof Error ? e.message : e) };
			}
		}
		return out;
	}

	/** Whole-file agreement verdict relative to tsv/prettier. */
	function verdict(out: Record<string, Out>): string {
		const tsv = out.tsv?.text;
		const pret = out.prettier?.text;
		const parts: string[] = [];
		const eq = (x?: string, y?: string) => x !== undefined && y !== undefined && x === y;
		parts.push(eq(tsv, pret) ? 'tsv==prettier' : 'tsv≠prettier');
		for (const name of ['biome', 'oxfmt']) {
			const t = out[name]?.text;
			if (t === undefined) {
				parts.push(`${name}:${out[name]?.err ? 'ERR' : '-'}`);
				continue;
			}
			const tags: string[] = [];
			if (eq(t, tsv)) tags.push('=tsv');
			if (eq(t, pret)) tags.push('=prettier');
			parts.push(`${name}${tags.length ? '[' + tags.join(',') + ']' : '[≠both]'}`);
		}
		return parts.join(' ');
	}

	const sources: Array<{ label: string; src: string }> = [];
	const files = a.files === undefined ? [] : [a.files, a._].flat();
	if (files.length > 0) {
		for (const f of files) sources.push({ label: f, src: await deps.read_file(f) });
	} else if (a.file) {
		sources.push({ label: a.file, src: await deps.read_file(a.file) });
	} else if (a.stdin) {
		sources.push({ label: '<stdin>', src: await deps.read_stdin() });
	} else if (a.content !== undefined) {
		sources.push({ label: '<content>', src: a.content });
	} else {
		console.error('provide --content, --stdin, --file, or --files');
		dispose();
		return 1;
	}

	const batch = sources.length > 1 || files.length > 0;
	for (const { label, src } of sources) {
		const out = await format_all(src);
		if (batch || a.quiet) {
			await print(`${verdict(out)}  ${label}`);
			continue;
		}
		const order = ['tsv', 'prettier', 'biome', 'oxfmt'];
		for (const name of order) {
			const o = out[name];
			if (!o) continue;
			await print(`\n===== ${name} =====`);
			await print(o.err ? `  <ERROR> ${o.err}` : (o.text ?? ''));
		}
		await print(`\n===== verdict =====\n${verdict(out)}`);
	}

	dispose();
	return 0;
}
