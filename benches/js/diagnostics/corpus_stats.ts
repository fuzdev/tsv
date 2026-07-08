/**
 * Corpus stats — size up the perf corpus, or a candidate directory before it
 * joins `CORPUS_ENTRIES`, so inclusion decisions go on numbers rather than a
 * hunch. Reports per-entry / per-subtree file counts, the language split, file
 * SIZE distribution (median/p90/p99/max — the current `corpus_sources`
 * disclosure only carries bare counts), the largest files, and the
 * concentration flags that expose degenerate cases (a giant generated data
 * file, a template/scaffolding tree, a fixture-like `tests/` subtree) which
 * skew a perf corpus meant to be representative real-world code.
 *
 * Reuses the REAL loaders/filters (`lib/corpus.ts`) so the numbers match what
 * the bench would actually measure — a directory is streamed via
 * `stream_perf_candidate` (the exact perf-view filtering: curated exclusions,
 * no build-output prune, the fixture/samples prune applied), not a raw walk.
 * Pure read-only; no parsing, no FFI. Deno-idiomatic like the other
 * `diagnostics/` entries.
 *
 *   deno task corpus:stats                     # perf view (the bench corpus)
 *   deno task corpus:stats --view gates        # + prettier fixture suites
 *   deno task corpus:stats ../language-tools/packages/svelte2tsx/src  # a candidate
 *   deno task corpus:stats ../cli/packages/sv/src --raw   # unfiltered walk (see what the prune drops)
 *   deno task corpus:stats --json 2>/dev/null  # machine-readable to stdout, logs to stderr
 *
 * Run directly (from repo root):
 *   deno run --allow-read --allow-env --allow-sys --config benches/js/deno.json \
 *     benches/js/diagnostics/corpus_stats.ts [dir] [--view v] [--raw] [--json] [--largest N] [--big BYTES]
 */

import { parseArgs } from 'node:util';
import { dirname, relative, resolve, sep } from 'node:path';

import { type CorpusView, DevReposLoader, DirectoryLoader, stream_perf_candidate } from '../lib/corpus.ts';
import { LANGUAGES, type Language, type SourceFile } from '../lib/types.ts';

/** Directory-name segments worth calling out when a candidate carries them — the
 * usual homes of non-representative bulk (generated data, scaffolding, snapshots,
 * fixture-like trees). `fixtures` / `test/samples` are already pruned by the
 * perf-candidate stream, so these are the ones that SURVIVE and need a human call. */
const WATCH_SEGMENTS = ['templates', 'template', 'tests', 'snapshots', '__snapshots__', 'generated', '__fixtures__'];

const SMALL_FILE_BYTES = 256;

interface Stats {
	files: number;
	bytes: number;
	by_language: Record<Language, number>;
	median: number;
	p90: number;
	p99: number;
	max: number;
}

function percentile(sorted: number[], q: number): number {
	if (sorted.length === 0) return 0;
	const k = Math.round((sorted.length - 1) * q);
	return sorted[k];
}

function stats_of(files: SourceFile[]): Stats {
	const sizes = files.map((f) => f.bytes).sort((a, b) => a - b);
	const by_language: Record<Language, number> = { svelte: 0, typescript: 0, css: 0 };
	let bytes = 0;
	for (const f of files) {
		by_language[f.language]++;
		bytes += f.bytes;
	}
	return {
		files: files.length,
		bytes,
		by_language,
		median: percentile(sizes, 0.5),
		p90: percentile(sizes, 0.9),
		p99: percentile(sizes, 0.99),
		max: sizes[sizes.length - 1] ?? 0,
	};
}

function fmt_bytes(n: number): string {
	if (n >= 1024 * 1024) return `${(n / 1024 / 1024).toFixed(2)} MB`;
	if (n >= 1024) return `${(n / 1024).toFixed(1)} KB`;
	return `${n} B`;
}

function lang_mix(by: Record<Language, number>): string {
	return LANGUAGES.filter((l) => by[l] > 0).map((l) => `${l[0]}:${by[l]}`).join(' ');
}

/** Every ancestor directory of `path` strictly under `root`, deepest last. */
function ancestors_under(path: string, root: string): string[] {
	const out: string[] = [];
	let dir = dirname(path);
	while (dir.length > root.length && dir.startsWith(root)) {
		out.push(dir);
		dir = dirname(dir);
	}
	return out;
}

/** Subtree file-count/byte concentration: each directory under `root` tagged with
 * how many corpus files live beneath it. Surfaces a bulk cluster (a `templates/`
 * or `tests/` tree) regardless of how deeply it nests. */
function concentration(
	files: SourceFile[],
	root: string,
	top: number,
): Array<{ dir: string; files: number; bytes: number }> {
	const count = new Map<string, number>();
	const bytes = new Map<string, number>();
	for (const f of files) {
		for (const dir of ancestors_under(f.path, root)) {
			count.set(dir, (count.get(dir) ?? 0) + 1);
			bytes.set(dir, (bytes.get(dir) ?? 0) + f.bytes);
		}
	}
	return [...count.entries()]
		.map(([dir, n]) => ({ dir: relative(root, dir), files: n, bytes: bytes.get(dir) ?? 0 }))
		.sort((a, b) => b.files - a.files)
		.slice(0, top);
}

/** WATCH_SEGMENT subtrees present in the set (name → files/bytes), for the flags. */
function watch_hits(files: SourceFile[]): Array<{ segment: string; files: number; bytes: number }> {
	const acc = new Map<string, { files: number; bytes: number }>();
	for (const f of files) {
		const segs = new Set(f.path.split(sep));
		for (const w of WATCH_SEGMENTS) {
			if (segs.has(w)) {
				const cur = acc.get(w) ?? { files: 0, bytes: 0 };
				cur.files++;
				cur.bytes += f.bytes;
				acc.set(w, cur);
			}
		}
	}
	return [...acc.entries()].map(([segment, v]) => ({ segment, ...v })).sort((a, b) => b.files - a.files);
}

async function collect_view(view: CorpusView): Promise<{ files: SourceFile[]; entry_paths: string[] }> {
	const loader = new DevReposLoader(view);
	const files = await loader.load(() => {}); // silent — this tool prints its own summary
	return { files, entry_paths: loader.sources.map((s) => resolve(s.path)) };
}

async function collect_dir(dir: string, raw: boolean): Promise<SourceFile[]> {
	const files: SourceFile[] = [];
	if (raw) {
		for await (const f of new DirectoryLoader(dir).stream(() => {})) files.push(f);
	} else {
		for await (const f of stream_perf_candidate(resolve(dir))) files.push(f);
	}
	return files;
}

/** Bucket files by the corpus entry (longest matching resolved-path prefix). */
function group_by_entry(
	files: SourceFile[],
	entry_paths: string[],
): Array<{ label: string; stats: Stats }> {
	const sorted_entries = [...entry_paths].sort((a, b) => b.length - a.length);
	const buckets = new Map<string, SourceFile[]>();
	for (const f of files) {
		const entry = sorted_entries.find((e) => f.path === e || f.path.startsWith(e + sep));
		const key = entry ?? '(unattributed)';
		(buckets.get(key) ?? buckets.set(key, []).get(key)!).push(f);
	}
	return [...buckets.entries()]
		.map(([abs, fs]) => ({ label: abs === '(unattributed)' ? abs : relative(resolve('..'), abs), stats: stats_of(fs) }))
		.sort((a, b) => b.stats.files - a.stats.files);
}

function main(): void {
	const { values, positionals } = parseArgs({
		args: Deno.args,
		allowPositionals: true,
		options: {
			view: { type: 'string', default: 'perf' },
			raw: { type: 'boolean', default: false },
			json: { type: 'boolean', default: false },
			largest: { type: 'string', default: '12' },
			big: { type: 'string', default: '65536' },
		},
	});
	const dir = positionals[0];
	const largest_n = Number(values.largest);
	const big = Number(values.big);
	const to_json = values.json;
	const log = (...a: unknown[]): void => console.error(...a); // human output → stderr (keeps --json stdout clean)

	run().catch((e) => {
		console.error(e instanceof Error ? e.message : String(e));
		Deno.exit(1);
	});

	async function run(): Promise<void> {
		let files: SourceFile[];
		let root: string;
		let title: string;
		let entry_paths: string[] = [];
		if (dir) {
			files = await collect_dir(dir, values.raw!);
			root = resolve(dir);
			title = `directory ${dir}${values.raw ? ' (raw walk)' : ' (perf-view filters)'}`;
		} else {
			const view = values.view as CorpusView;
			if (!['perf', 'gates', 'conformance'].includes(view)) {
				throw new Error(`--view must be perf|gates|conformance (got '${view}')`);
			}
			({ files, entry_paths } = await collect_view(view));
			root = resolve('..');
			title = `${view} view`;
		}
		if (files.length === 0) throw new Error(`No in-scope files found for ${title}`);

		const overall = stats_of(files);
		const per_lang = LANGUAGES.map((l) => ({ language: l, stats: stats_of(files.filter((f) => f.language === l)) }))
			.filter((r) => r.stats.files > 0);
		const groups = dir ? [] : group_by_entry(files, entry_paths);
		const conc = dir ? concentration(files, root, largest_n) : [];
		const big_files = files.filter((f) => f.bytes > big).sort((a, b) => b.bytes - a.bytes);
		const small_files = files.filter((f) => f.bytes < SMALL_FILE_BYTES).length;
		const watch = watch_hits(files);
		const largest = [...files].sort((a, b) => b.bytes - a.bytes).slice(0, largest_n);

		if (to_json) {
			console.log(JSON.stringify({
				title,
				overall,
				per_language: per_lang,
				groups,
				concentration: conc,
				largest: largest.map((f) => ({ path: relative(root, f.path), language: f.language, bytes: f.bytes })),
				flags: {
					big_bytes: big,
					big_files: big_files.map((f) => ({ path: relative(root, f.path), bytes: f.bytes })),
					small_files: { under_bytes: SMALL_FILE_BYTES, count: small_files },
					watch_segments: watch,
				},
			}, null, 2));
			return;
		}

		log(`\nCorpus stats — ${title}`);
		log(`\nOverall: ${overall.files} files, ${fmt_bytes(overall.bytes)}`);
		for (const { language, stats } of per_lang) {
			log(
				`  ${language.padEnd(11)} ${String(stats.files).padStart(5)} files  ${fmt_bytes(stats.bytes).padStart(9)}` +
					`   median ${fmt_bytes(stats.median)}  p90 ${fmt_bytes(stats.p90)}  p99 ${fmt_bytes(stats.p99)}  max ${fmt_bytes(stats.max)}`,
			);
		}

		if (groups.length > 0) {
			log(`\nPer entry (${groups.length}):`);
			for (const g of groups) {
				log(
					`  ${g.label.padEnd(42)} ${String(g.stats.files).padStart(4)}f  ${fmt_bytes(g.stats.bytes).padStart(9)}` +
						`  [${lang_mix(g.stats.by_language)}]  med ${fmt_bytes(g.stats.median)}  p99 ${fmt_bytes(g.stats.p99)}`,
				);
			}
		}
		if (conc.length > 0) {
			log(`\nSubtree concentration (files beneath each dir):`);
			for (const c of conc) {
				log(`  ${String(c.files).padStart(4)}f  ${fmt_bytes(c.bytes).padStart(9)}  ${c.dir || '.'}`);
			}
		}

		log(`\nLargest ${largest.length} files:`);
		for (const f of largest) {
			log(`  ${fmt_bytes(f.bytes).padStart(9)}  ${f.language.padEnd(10)} ${relative(root, f.path)}`);
		}

		log(`\nFlags:`);
		log(
			big_files.length > 0
				? `  ⚠ ${big_files.length} file(s) > ${fmt_bytes(big)} (largest ${fmt_bytes(big_files[0].bytes)} — ${relative(root, big_files[0].path)})`
				: `  ✓ no files > ${fmt_bytes(big)}`,
		);
		log(`  small files (< ${SMALL_FILE_BYTES} B): ${small_files} (${((small_files / overall.files) * 100).toFixed(0)}%)`);
		if (watch.length > 0) {
			for (const w of watch) {
				log(`  ⚠ '${w.segment}/' subtree present: ${w.files} files, ${fmt_bytes(w.bytes)} — likely non-representative; consider excluding`);
			}
		} else {
			log(`  ✓ no ${WATCH_SEGMENTS.join('/')} subtrees`);
		}
		log('');
	}
}

main();
