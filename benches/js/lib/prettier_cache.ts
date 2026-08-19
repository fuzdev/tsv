/**
 * Content-addressed prettier-output cache — the corpus format comparison's
 * dominant cost is re-running prettier over ~6k mostly-unchanged files, so
 * successful outputs are cached under `benches/js/.cache/prettier/` keyed by
 * everything that determines them.
 *
 * **Key completeness** (prettier is deterministic given these, so a hit is
 * exactly equivalent to a live run):
 *
 * - the SOURCE CONTENT (hashed);
 * - the PARSER + synthetic FILEPATH (`file<ext>`) — the routing inputs
 *   `CanonicalImplementation.format_async` derives (`.js`→babel etc.);
 * - the full PRETTIER OPTIONS object (serialized);
 * - the CANONICAL-5 npm pin versions — prettier, prettier-plugin-svelte, AND
 *   svelte (the plugin's peer, proven output-affecting by a past svelte bump's
 *   convergence change), plus acorn/@sveltejs/acorn-typescript for
 *   over-invalidation safety. npm pins are exact and installed==declared is
 *   enforced by the `node_modules` staleness gate, so VERSIONS fully identify
 *   the oracle — checkout COMMITS matter only for checkout-based inputs, which
 *   the harvest stamps cover (`lib/harvest_stamp.ts`);
 * - the `PRETTIER_DEBUG` flag (flips the svelte plugin's error-vs-verbatim
 *   fallback, changing which files error);
 * - a `CACHE_SCHEMA` constant (bump to invalidate wholesale).
 *
 * **Success-only**: thrown errors are never cached (a flaky sidecar-class
 * failure must not freeze), and empty outputs are never cached (the
 * empty-output prettier-miss heisenbug is guarded by the caller — see
 * benches/js/CLAUDE.md §Known Issues — so a miss can't poison the cache). A
 * side effect worth knowing: cached hits remove the prettier-side flake from
 * repeat runs entirely — only the tsv/FFI side stays live.
 *
 * **Atomic**: entries land by `rename` from a PID-suffixed temp file, so an
 * interrupted run can never leave a TRUNCATED entry at a content-addressed key —
 * which the empty guards above cannot catch and which every later run would read
 * as the oracle itself. See `put`.
 *
 * **Scope**: opt-in via `CanonicalImplementation.enable_format_cache()` — used
 * by `corpus:compare:format` and the `conformance.ts` driver only. NEVER the
 * bench (it times prettier) and NEVER the fixture validator (live-verification
 * by design). Disable for a session with `TSV_PRETTIER_CACHE=0`; wiped by
 * `deno task bench:clean`; unbounded (content-addressed, no eviction) — clean
 * when it bothers you.
 */

import { createHash } from 'node:crypto';
import { mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import process from 'node:process';

import type { CanonicalVersions } from './versions.ts';

const CACHE_DIR = 'benches/js/.cache/prettier';
const CACHE_SCHEMA = 1;

/** Whether the cache is enabled for this session (`TSV_PRETTIER_CACHE=0` disables). */
export function prettier_cache_enabled(): boolean {
	return process.env.TSV_PRETTIER_CACHE !== '0';
}

export class PrettierCache {
	/** Hash of every non-per-file key input — see the module docstring. */
	readonly #context_hash: string;
	hits = 0;
	misses = 0;
	/** Entries `put` could not write (see its doc) — reported, never thrown. */
	writes_failed = 0;

	constructor(versions: CanonicalVersions, prettier_options_json: string) {
		this.#context_hash = createHash('sha256')
			.update(
				JSON.stringify({
					schema: CACHE_SCHEMA,
					versions,
					options: prettier_options_json,
					prettier_debug: process.env.PRETTIER_DEBUG ?? null
				})
			)
			.digest('hex');
	}

	/** Sharded entry location (`<CACHE_DIR>/<h[0..2]>/<h>`). */
	#entry(source: string, parser: string, filepath: string): { dir: string; path: string } {
		const h = createHash('sha256')
			.update(this.#context_hash)
			.update('\0')
			.update(parser)
			.update('\0')
			.update(filepath)
			.update('\0')
			.update(source)
			.digest('hex');
		const dir = join(CACHE_DIR, h.slice(0, 2));
		return { dir, path: join(dir, h) };
	}

	async get(source: string, parser: string, filepath: string): Promise<string | null> {
		try {
			const output = await readFile(this.#entry(source, parser, filepath).path, 'utf8');
			if (output.trim() === '') {
				// semantically-empty entry (predates the put() trim guard) — treat as
				// a miss so a poisoned entry can never freeze a bad oracle
				this.misses++;
				return null;
			}
			this.hits++;
			return output;
		} catch {
			this.misses++;
			return null;
		}
	}

	/**
	 * Record a SUCCESSFUL, semantically NON-EMPTY output (guards enforced here
	 * too). Whitespace-only counts as empty: the safety differential strips
	 * whitespace, so caching such an output would freeze exactly the
	 * prettier-miss shape the empty guard exists to reject.
	 *
	 * ATOMIC — written to a sibling temp name and renamed into place. A direct
	 * write is not interruption-safe, and an interrupted one is worse than a miss:
	 * the truncated bytes sit at a content-addressed key, so every later run reads
	 * a PREFIX of prettier's output as the oracle itself. That poisons the release
	 * gate this cache serves (`corpus:compare:format --all`), and the `trim()`
	 * guards catch only a fully-empty entry, never a truncated one. `rename` within
	 * a directory is atomic, so a reader sees the whole entry or no entry.
	 *
	 * The temp name carries the PID so two concurrent writers of the same key
	 * (different processes, same corpus) can't truncate each other's temp file.
	 *
	 * BEST-EFFORT — a write failure is counted, never thrown. The caller already
	 * holds the live output, and its `format_async` reads a throw as "prettier
	 * failed on this file", so letting a full disk out of here would file cache
	 * trouble in the gate's per-file ERROR bucket. `writes_failed` is what keeps
	 * that from being silent: a wholly unwritable cache dir shows up in `stats()`
	 * as every put failing rather than as a cache that never warms.
	 */
	async put(source: string, parser: string, filepath: string, output: string): Promise<void> {
		if (output.trim() === '') return;
		const { dir, path } = this.#entry(source, parser, filepath);
		const tmp = `${path}.${process.pid}.tmp`;
		try {
			await mkdir(dir, { recursive: true });
			await writeFile(tmp, output);
			await rename(tmp, path);
		} catch {
			this.writes_failed++;
			await rm(tmp, { force: true }).catch(() => {});
		}
	}

	stats(): string {
		const failed = this.writes_failed > 0 ? ` / ${this.writes_failed} writes FAILED` : '';
		return `prettier cache: ${this.hits} hits / ${this.misses} misses${failed}`;
	}
}
