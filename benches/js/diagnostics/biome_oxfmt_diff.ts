/**
 * 4-way formatter differential triage: tsv (FFI) vs prettier(-typescript) vs
 * biome-wasm vs oxfmt. A hand-triage aid for the Biome/oxfmt corpus-output
 * mining pass (see grimoire TODO_BIOME_PRETTIER_DIFFS.md) — NOT a gate.
 *
 * The corpus:compare:format tool only diffs tsv vs prettier; this adds biome +
 * oxfmt as third/fourth opinions so a tsv-vs-prettier divergence can be bucketed:
 *   - tsv alone (prettier == biome == oxfmt, all differ from tsv) → candidate bug
 *   - tsv + biome (or + oxfmt) agree vs prettier → candidate sanctioned divergence
 *
 * Prettier is routed through the TYPESCRIPT parser (filepath `snippet.ts`), never
 * babel — the item-2..5 guardrail ([[biome-diffs-use-babel-recheck-typescript]]).
 *
 * Usage (from repo root):
 *   deno run --allow-ffi --allow-read --allow-env --allow-net --allow-sys \
 *     benches/js/diagnostics/biome_oxfmt_diff.ts --parser typescript --content '<code>'
 *   echo '<code>' | deno run ... biome_oxfmt_diff.ts --parser typescript --stdin
 *   deno run ... biome_oxfmt_diff.ts --parser typescript --file path/to/file.ts
 *   deno run ... biome_oxfmt_diff.ts --parser typescript --files a.ts b.ts   # batch verdicts, no bodies
 */

import { args_parse, argv_parse } from '@fuzdev/fuz_util/args.ts';
import { z } from 'zod';

import { CanonicalImplementation } from '../lib/canonical.ts';
import { NativeImplementation } from '../lib/ffi.ts';
import { BiomeImplementation } from '../lib/biome.ts';
import { OxcImplementation } from '../lib/oxc.ts';
import { load_all_versions } from '../lib/versions.ts';
import { type Language, LANGUAGE_EXTENSIONS } from '../lib/types.ts';

const Args = z.object({
	_: z.array(z.string()).default(() => []),
	parser: z.enum(['typescript', 'svelte', 'css']).default('typescript'),
	content: z.string().optional(),
	file: z.string().optional(),
	files: z.array(z.string()).optional(),
	stdin: z.boolean().default(false),
	quiet: z.boolean().default(false),
});

type Out = { text?: string; err?: string };

async function main() {
	const args = args_parse(argv_parse(Deno.args), Args);
	if (!args.success) {
		console.error(z.prettifyError(args.error));
		Deno.exit(1);
	}
	const a = args.data;
	const lang = a.parser as Language;
	const ext = LANGUAGE_EXTENSIONS[lang];

	const versions = await load_all_versions();
	const canonical = new CanonicalImplementation(versions.canonical);
	const native = new NativeImplementation();
	const biome = new BiomeImplementation(versions.biome);
	const oxc = new OxcImplementation(versions.oxc);
	await canonical.init();
	await native.init();
	let biome_ok = true;
	let oxc_ok = true;
	try {
		await biome.init();
	} catch (e) {
		biome_ok = false;
		console.error(`biome unavailable: ${e}`);
	}
	try {
		await oxc.init();
	} catch (e) {
		oxc_ok = false;
		console.error(`oxfmt unavailable: ${e}`);
	}

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
		if (biome_ok && biome.supports_format_language(lang)) {
			try {
				out.biome = { text: biome.format(src, lang) };
			} catch (e) {
				out.biome = { err: String(e instanceof Error ? e.message : e) };
			}
		}
		if (oxc_ok && oxc.supports_format_language(lang)) {
			try {
				out.oxfmt = { text: await oxc.format_async(src, lang) };
			} catch (e) {
				out.oxfmt = { err: String(e instanceof Error ? e.message : e) };
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
	if (a.files && a.files.length) {
		for (const f of a.files) sources.push({ label: f, src: Deno.readTextFileSync(f) });
	} else if (a.file) {
		sources.push({ label: a.file, src: Deno.readTextFileSync(a.file) });
	} else if (a.stdin) {
		const buf = new Uint8Array(1 << 20);
		const n = Deno.stdin.readSync(buf) ?? 0;
		sources.push({ label: '<stdin>', src: new TextDecoder().decode(buf.subarray(0, n)) });
	} else if (a.content !== undefined) {
		sources.push({ label: '<content>', src: a.content });
	} else {
		console.error('provide --content, --stdin, --file, or --files');
		Deno.exit(1);
	}

	const batch = sources.length > 1 || !!a.files;
	for (const { label, src } of sources) {
		const out = await format_all(src);
		if (batch || a.quiet) {
			console.log(`${verdict(out)}  ${label}`);
			continue;
		}
		const order = ['tsv', 'prettier', 'biome', 'oxfmt'];
		for (const name of order) {
			const o = out[name];
			if (!o) continue;
			console.log(`\n===== ${name} =====`);
			console.log(o.err ? `  <ERROR> ${o.err}` : o.text);
		}
		console.log(`\n===== verdict =====\n${verdict(out)}`);
	}

	canonical.dispose();
	native.dispose();
	if (biome_ok) biome.dispose();
	if (oxc_ok) oxc.dispose();
}

main();
