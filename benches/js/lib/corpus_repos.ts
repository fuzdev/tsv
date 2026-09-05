/**
 * Reify each corpus source's origin repo as a typed {@link CorpusRepoRef} so the
 * report is self-describing — the site links straight to
 * `https://github.com/<slug>/tree/<commit>/<subpath>` without a hand-maintained
 * path→URL map (which drifts from the corpus).
 *
 * Two kinds of source, two detections:
 *
 * - A **snapshot collection** (`../corpora/collections/<name>/…`) links to its
 *   UPSTREAM at the commit the snapshot vendored — read from the snapshot's own
 *   `manifest.json` (`lib/corpora.ts`), so the link names the code that was
 *   measured, not the vendoring repo. The snapshot itself is reported once, as the
 *   report's `corpus_snapshot` ({@link detect_corpus_snapshot}): one roll-up commit
 *   for every real-code source.
 * - Any other checkout is **git-detected**: the URL from its `origin` remote and the
 *   commit from `HEAD`, so the link pins to the exact code and can't fall out of
 *   sync with the corpus entries (`corpus_entries`).
 *
 * The only declared data is {@link CLONE_URL_BY_PREFIX} — the canonical clone URLs
 * for the *absent* checkouts a fresh machine is missing, where nothing can be
 * detected (see {@link clone_hint}).
 *
 * Runtime-neutral (`node:child_process`, `node:fs`, like `binary_sizes.ts`), so it
 * runs under both the Deno and Node bench drivers; needs `--allow-run=git` under
 * Deno.
 */

import { execFile } from 'node:child_process';
import { dirname, relative, resolve } from 'node:path';
import { promisify } from 'node:util';

import {
	CORPORA_ROOT,
	CORPORA_URL,
	is_collection_path,
	load_corpora_manifest,
	split_collection_path
} from './corpora.ts';
import type { CorpusRepoRef, CorpusSource } from './corpus.ts';

const exec_file = promisify(execFile);

/**
 * Sources under `benches/js/.cache` are HARVESTED from an upstream repo, so
 * git-detecting their in-tree path resolves the tsv repo — and the local
 * `../wpt` / `../test262` checkouts are typically personal *forks*, which read
 * oddly on a public page. Link the CANONICAL upstream at its root instead (the
 * harvest is a subset, so a commit-pinned deep link isn't meaningful): a
 * declared URL, no commit.
 */
const CACHE_CANONICAL: Record<string, string> = {
	'benches/js/.cache/wpt_css': 'https://github.com/web-platform-tests/wpt',
	'benches/js/.cache/test262_files.json': 'https://github.com/tc39/test262',
	'benches/js/.cache/ts_repo_files.json': 'https://github.com/microsoft/TypeScript'
};

/**
 * Canonical clone URLs for the snapshot + suite checkouts, keyed by path prefix.
 * Used ONLY for {@link clone_hint} — the triage message for an ABSENT checkout,
 * where nothing can be detected. The one `../corpora` line covers every real-code
 * source, which is the point of the snapshot.
 */
const CLONE_URL_BY_PREFIX: Array<readonly [string, string]> = [
	[CORPORA_ROOT, CORPORA_URL],
	['../prettier-plugin-svelte', 'https://github.com/sveltejs/prettier-plugin-svelte'],
	['../prettier', 'https://github.com/prettier/prettier'],
	['../svelte', 'https://github.com/sveltejs/svelte'],
	['../acorn-typescript', 'https://github.com/sveltejs/acorn-typescript'],
	['../typescript', 'https://github.com/microsoft/TypeScript'],
	['../wpt', 'https://github.com/web-platform-tests/wpt'],
	['../test262', 'https://github.com/tc39/test262']
];

async function git(cwd: string, args: string[]): Promise<string | null> {
	try {
		const { stdout } = await exec_file('git', args, { cwd });
		const out = stdout.trim();
		return out.length > 0 ? out : null;
	} catch {
		return null;
	}
}

/**
 * `git@github.com:org/repo(.git)` / `https://github.com/org/repo(.git)` →
 * `https://github.com/org/repo`; `null` for a non-GitHub or unparseable remote.
 */
function normalize_github_url(remote: string | null): string | null {
	if (!remote) return null;
	const match = remote.match(/github\.com[:/]([^/]+)\/(.+?)(?:\.git)?\/?$/);
	return match ? `https://github.com/${match[1]}/${match[2]}` : null;
}

/** `owner/name` from a canonical GitHub URL. */
function slug_of(url: string): string {
	return new URL(url).pathname.slice(1);
}

/**
 * The top-level checkout prefix for `source_path` (the segment `clone_hint` keys on)
 * — the first two segments of a `../repo/...` path.
 */
function checkout_prefix(source_path: string): string {
	const parts = source_path.split('/');
	return parts[0] === '..' ? `${parts[0]}/${parts[1]}` : parts[0];
}

/**
 * A snapshot collection's upstream ref: `../corpora/collections/kit/packages/kit/src`
 * → sveltejs/kit at the vendored commit, subpath `packages/kit/src`. `null` when the
 * path is not a collection or the manifest can't answer (the caller then git-detects,
 * which links the snapshot repo itself — a correct if less specific link).
 */
async function detect_collection_upstream(source_path: string): Promise<CorpusRepoRef | null> {
	const split = split_collection_path(source_path);
	if (!split) return null;
	const manifest = await load_corpora_manifest();
	const collection = manifest.status === 'ok' ? manifest.collections.get(split.name) : undefined;
	if (!collection) return null;
	return {
		url: collection.url,
		slug: slug_of(collection.url),
		commit: collection.commit,
		subpath: split.subpath
	};
}

/** Git-detect the GitHub ref of the checkout containing `abs` (a directory). */
async function detect_checkout(abs: string): Promise<CorpusRepoRef | null> {
	const toplevel = await git(abs, ['rev-parse', '--show-toplevel']);
	if (!toplevel) return null;
	const [commit, remote] = await Promise.all([
		git(toplevel, ['rev-parse', 'HEAD']),
		git(toplevel, ['remote', 'get-url', 'origin'])
	]);
	const url = normalize_github_url(remote);
	if (!url || !commit) return null;
	return { url, slug: slug_of(url), commit, subpath: relative(toplevel, abs) };
}

/** Detect the GitHub ref for one corpus source path (present repos only). */
async function detect_repo(source_path: string): Promise<CorpusRepoRef | null> {
	// A harvested cache links to its declared canonical upstream at the root
	// (no git, no commit) — see `CACHE_CANONICAL`.
	const canonical = CACHE_CANONICAL[source_path];
	if (canonical) {
		return { url: canonical, slug: slug_of(canonical), commit: '', subpath: '' };
	}
	// Any other derived cache (e.g. `svelte_styles`) is gitignored: git would
	// resolve the enclosing tsv repo and mint a dead link.
	//
	// ⚠ A cache HARVESTED from an upstream repo belongs in `CACHE_CANONICAL`
	// above, not here — this arm is for the locally-derived ones. Falling through
	// costs nothing at run time and fails silently downstream: the source publishes
	// as a bare in-repo path beside siblings that link out. A new harvest earns its
	// entry in the same change.
	if (source_path.startsWith('benches/js/.cache')) return null;

	// A snapshot collection names its upstream; anything else is whatever checkout
	// holds it (for a collection with no manifest answer, that is the snapshot repo).
	const upstream = await detect_collection_upstream(source_path);
	if (upstream) return upstream;

	const abs = resolve(source_path);
	// `git -C` needs a directory; a `files_from` entry may point at a file.
	return detect_checkout(/\.[a-z]+$/i.test(source_path) ? dirname(abs) : abs);
}

/**
 * Populate `source.repo` for every source (manifest- or git-detected; left
 * `undefined` for a source with no GitHub origin, e.g. the local `svelte_styles`
 * cache). Runs the detections concurrently — a handful of cheap `git` calls at
 * report-build time.
 */
export async function enrich_source_repos(sources: CorpusSource[]): Promise<void> {
	await Promise.all(
		sources.map(async (source) => {
			source.repo = (await detect_repo(source.path)) ?? undefined;
		})
	);
}

/**
 * The real-code snapshot the report was measured against — `fuzdev/corpora` at the
 * checked-out commit (subpath `''`), git-detected — or `null` when no loaded source
 * is one of its collections (a conformance-only run reads no real code, so the field
 * must not name a snapshot the run never opened) or the checkout is absent. One
 * roll-up commit for every `real` / `framework` source, so a reader can reproduce
 * the corpus with one clone. The commit, deliberately, where the corpus gates pin
 * the checkout's `collections/` tree id (`GATE_CHECKOUT_IDS`): a tree id names
 * the bytes but cannot be cloned, and the commit fixes the tree — so the two agree,
 * though a tooling commit in the snapshot repo moves this without moving that.
 */
export async function detect_corpus_snapshot(
	sources: readonly CorpusSource[]
): Promise<CorpusRepoRef | null> {
	if (!sources.some((s) => is_collection_path(s.path))) return null;
	return detect_checkout(resolve(CORPORA_ROOT));
}

/**
 * A `git clone <url> <dir>` triage line for an ABSENT corpus checkout, or `null`
 * when its URL isn't declared (a `live` working tree — cloning the author's dev
 * repos isn't the fresh-machine story the snapshot and suite checkouts are). `<dir>`
 * is the checkout root the entry lives under (e.g. `../svelte`).
 */
export function clone_hint(source_path: string): string | null {
	// Match the checkout root exactly, not by raw string prefix — `../svelte` is a
	// character prefix of `../svelte-docinfo/src` (an unrelated live entry),
	// which would hint cloning the Svelte framework into `../svelte-docinfo`.
	const dir = checkout_prefix(source_path);
	const match = CLONE_URL_BY_PREFIX.find(([prefix]) => prefix === dir);
	return match ? `git clone ${match[1]} ${dir}` : null;
}
