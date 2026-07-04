/**
 * Harvest the test262 graded-positive file list for the conformance corpus.
 *
 * Runs `tsv_debug test262 --emit-manifest` (the Rust harness grades tsv's
 * strict subset from each test's front-matter: `negative:` expectations,
 * `noStrict`/`raw` sloppy exclusions, `module` flags), then filters to the
 * EXPECTED-POSITIVE tests and writes the path list the corpus loader's
 * `files_from` entry consumes. The filter is tool-neutral by construction —
 * test262's own metadata decides, never tsv's verdict, so the subset can't
 * bias per-tool coverage toward tsv.
 *
 * The bench parses these files at every tool's default (module) goal — the
 * manifest's per-file `module` flag is not threaded (none of the tsv bindings
 * take a goal parameter, and the canonical acorn wrapper hardcodes
 * `sourceType: 'module'`), so strict-script-only constructs (e.g. `await` as
 * an identifier) count against every tool equally. The goal-aware differential
 * is `diagnostics/test262_compare.ts`.
 *
 * Output:
 * - benches/js/.cache/test262_files.json — sorted array of project-root-relative paths
 * - benches/js/.cache/test262_manifest.json — the raw manifest (kept for inspection)
 *
 * Flags: `--if-present` tolerates a missing `../test262` checkout (warn +
 * exit 0) — for the `bench:conformance` task chain, where an absent suite
 * should produce a smaller corpus (disclosed via the report's
 * `corpus_sources`), not a failed bench.
 *
 * Run from the repo root:
 *   deno run --allow-read --allow-write --allow-run=cargo benches/js/harvest_test262.ts
 */

import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import { join } from 'node:path';

const TEST262_ROOT = '../test262';
const CACHE_DIR = 'benches/js/.cache';
const MANIFEST_PATH = `${CACHE_DIR}/test262_manifest.json`;
const FILES_PATH = `${CACHE_DIR}/test262_files.json`;

const if_present = Deno.args.includes('--if-present');

try {
	await stat(join(TEST262_ROOT, 'test'));
} catch {
	if (if_present) {
		console.error(
			`test262 checkout not found at ${TEST262_ROOT} — skipping harvest (--if-present)`,
		);
		Deno.exit(0);
	}
	console.error(`test262 checkout not found: ${TEST262_ROOT}`);
	Deno.exit(1);
}

await mkdir(CACHE_DIR, { recursive: true });

// --release: the harness parses the whole ~51k-file tree to grade it; a debug
// build is an order of magnitude slower.
console.error('Grading test262 (cargo run --release -p tsv_debug test262 --emit-manifest)…');
const command = new Deno.Command('cargo', {
	args: [
		'run',
		'--release',
		'-p',
		'tsv_debug',
		'--',
		'test262',
		'--path',
		TEST262_ROOT,
		'--emit-manifest',
		MANIFEST_PATH,
	],
	stdout: 'inherit',
	stderr: 'inherit',
});
const status = await command.output();
if (!status.success) {
	console.error('tsv_debug test262 --emit-manifest failed');
	Deno.exit(1);
}

interface ManifestEntry {
	relative_path: string;
	module: boolean;
	strict: boolean;
	expected: 'accept' | 'reject';
	tsv: 'accept' | 'reject';
}

interface Manifest {
	test262_root: string;
	count: number;
	tests: ManifestEntry[];
}

const manifest = JSON.parse(await readFile(MANIFEST_PATH, 'utf8')) as Manifest;
const positives = manifest.tests.filter((t) => t.expected === 'accept');
const files = positives.map((t) => join(manifest.test262_root, t.relative_path)).sort();
await writeFile(FILES_PATH, JSON.stringify(files, null, '\t'));

const module_count = positives.filter((t) => t.module).length;
console.error(
	`test262 harvest: ${files.length} expected-positive of ${manifest.count} graded tests ` +
		`(${module_count} module-flagged) → ${FILES_PATH}`,
);
