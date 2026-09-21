/**
 * The canonical toolchain's size rows: minified, tree-shaken JS bundles of prettier
 * and the canonical parsers, built so the size table has something to weigh against
 * tsv's artifacts.
 *
 * Every other row in that table is a file a package SHIPS. The canonical tools ship
 * no such file — `node_modules/prettier` holds every language plugin twice (ESM +
 * CJS), and `svelte` a whole compiler and runtime — so the installed size answers a
 * different question. What is comparable is the minimum a consumer would deploy:
 * one bundle per capability, scope-matched to tsv's three builds
 * (`size_bundles/canonical_{parse,format,all}.ts` state each scope). These are
 * SYNTHESIZED artifacts, and every label says `js bundle` so no reader takes one
 * for a published file.
 *
 * Built at collection time rather than by a build task: bundling all three takes
 * about a second, and a bundle made in the same run cannot be stale against the
 * installed pins — the hazard the freshness guard exists for on the expensive
 * builds. The bundler is `deno bundle` (esbuild underneath), spawned as a
 * subprocess so the collector stays runtime-neutral; Deno is the harness's one hard
 * dependency, so it is on PATH under Node and Bun too. Its esbuild moves with the
 * Deno version, so a row can shift by a few bytes across a Deno upgrade with no pin
 * moving. A bundle that fails to build is an ABSENT row, never a fatal one — the
 * size table is existence-gated throughout.
 */

import { execFile } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

const exec_file = promisify(execFile);

/** One canonical size bundle: the row's label and the entry that defines its scope. */
export interface CanonicalBundle {
	label: string;
	/** Entry module under `size_bundles/`, without extension — also the output's name. */
	entry: string;
}

/**
 * The three bundles, one per capability the site groups sizes by. The labels are
 * row identities the site's capability table keys on (tsv.fuz.dev
 * `benchmark_sizes.ts` `SIZE_CAPABILITY_BY_LABEL`) — rename there in the same change.
 */
export const CANONICAL_BUNDLES = {
	parse: { label: 'svelte + acorn-typescript parsers (js bundle)', entry: 'canonical_parse' },
	format: { label: 'prettier + svelte plugin (js bundle)', entry: 'canonical_format' },
	all: { label: 'prettier + parsers (js bundle)', entry: 'canonical_all' }
} as const satisfies Record<string, CanonicalBundle>;

const ENTRY_DIR = fileURLToPath(new URL('../size_bundles', import.meta.url));
const OUT_DIR = fileURLToPath(new URL('../.cache/size_bundles', import.meta.url));

/**
 * Bundle one entry, returning the output path or `null` if the build failed
 * (`deno` unreachable from this runtime, run permission withheld, a resolution
 * error). `--platform browser` because the browser build is the deployable minimum;
 * the node platform's output differs by ~0.1%.
 */
async function build_bundle(bundle: CanonicalBundle): Promise<string | null> {
	// `.js` on purpose: esbuild reads the output extension when it folds its ESM-interop
	// helper, so another name sizes a few bytes off what a consumer's build would emit.
	// (`deno.json` excludes `.cache`, or `deno check benches/js` would typecheck this.)
	const out = `${OUT_DIR}/${bundle.entry}.js`;
	try {
		await exec_file(
			'deno',
			[
				'bundle',
				'--quiet',
				'--minify',
				'--platform',
				'browser',
				'--config',
				fileURLToPath(new URL('../deno.json', import.meta.url)),
				'-o',
				out,
				`${ENTRY_DIR}/${bundle.entry}.ts`
			],
			{ maxBuffer: 16 * 1024 * 1024 }
		);
		return out;
	} catch (error) {
		// The report records only THAT the row is absent; the cause lives here. A missing
		// `deno` and a broken entry are different repairs and read alike without it.
		const reason = (error instanceof Error ? error.message : String(error)).split('\n')[0];
		console.error(`⚠ size bundle "${bundle.label}" not built: ${reason}`);
		return null;
	}
}

/**
 * Build every canonical bundle, in parallel. `path` is `null` for one that failed.
 *
 * The subprocess does every write, creating the output directory itself — so the
 * harness needs only run permission for `deno`, and no write access to the cache.
 */
export async function build_canonical_bundles(): Promise<
	Array<{ bundle: CanonicalBundle; path: string | null }>
> {
	return Promise.all(
		Object.values(CANONICAL_BUNDLES).map(async (bundle) => ({
			bundle,
			path: await build_bundle(bundle)
		}))
	);
}
