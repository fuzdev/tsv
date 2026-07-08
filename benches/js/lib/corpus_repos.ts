/**
 * Reify each corpus source's origin repo as a typed {@link CorpusRepoRef} so the
 * report is self-describing — the site links straight to
 * `https://github.com/<slug>/tree/<commit>/<subpath>` without a hand-maintained
 * path→URL map (which drifts from the corpus).
 *
 * **Detected, not declared** for present repos: the URL comes from the checkout's
 * `origin` remote and the commit from `HEAD`, so a link pins to the exact code the
 * numbers were measured against and can't fall out of sync with `CORPUS_ENTRIES`.
 * The only declared data is {@link CLONE_URL_BY_PREFIX} — the canonical clone URLs
 * for the *absent* checkouts a fresh machine is missing, where git can detect
 * nothing (see {@link clone_hint}).
 *
 * Runtime-neutral (`node:child_process`, like `binary_sizes.ts`), so it runs under
 * both the Deno and Node bench drivers; needs `--allow-run=git` under Deno.
 */

import { execFile } from 'node:child_process';
import { dirname, relative, resolve } from 'node:path';
import { promisify } from 'node:util';

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
};

/**
 * Canonical clone URLs for the suite/framework checkouts, keyed by path prefix.
 * Used ONLY for {@link clone_hint} — the triage message for an ABSENT checkout,
 * where git can detect nothing. Present repos get their URL from git instead
 * (so this list never has to enumerate the author's own `real`-tier repos).
 */
const CLONE_URL_BY_PREFIX: Array<readonly [string, string]> = [
	['../prettier-plugin-svelte', 'https://github.com/sveltejs/prettier-plugin-svelte'],
	['../prettier', 'https://github.com/prettier/prettier'],
	['../svelte.dev', 'https://github.com/sveltejs/svelte.dev'],
	['../svelte', 'https://github.com/sveltejs/svelte'],
	['../kit', 'https://github.com/sveltejs/kit'],
	['../acorn-typescript', 'https://github.com/sveltejs/acorn-typescript'],
	['../typescript', 'https://github.com/microsoft/TypeScript'],
	['../wpt', 'https://github.com/web-platform-tests/wpt'],
	['../test262', 'https://github.com/tc39/test262'],
];

async function git(cwd: string, args: string[]): Promise<string | null> {
	try {
		const {stdout} = await exec_file('git', args, {cwd});
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
export function normalize_github_url(remote: string | null): string | null {
	if (!remote) return null;
	const match = remote.match(/github\.com[:/]([^/]+)\/(.+?)(?:\.git)?\/?$/);
	return match ? `https://github.com/${match[1]}/${match[2]}` : null;
}

/**
 * The top-level checkout prefix for `source_path` (the segment `clone_hint` and
 * `CACHE_UPSTREAM` key on) — the first two segments of a `../repo/...` path.
 */
function checkout_prefix(source_path: string): string {
	const parts = source_path.split('/');
	return parts[0] === '..' ? `${parts[0]}/${parts[1]}` : parts[0];
}

/** Detect the GitHub ref for one corpus source path (present repos only). */
export async function detect_repo(source_path: string): Promise<CorpusRepoRef | null> {
	// A harvested cache links to its declared canonical upstream at the root
	// (no git, no commit) — see `CACHE_CANONICAL`.
	const canonical = CACHE_CANONICAL[source_path];
	if (canonical) {
		return {url: canonical, slug: new URL(canonical).pathname.slice(1), commit: '', subpath: ''};
	}
	// Any other derived cache (e.g. `svelte_styles`) is gitignored: git would
	// resolve the enclosing tsv repo and mint a dead link.
	if (source_path.startsWith('benches/js/.cache')) return null;

	const abs = resolve(source_path);
	// `git -C` needs a directory; a `files_from` entry may point at a file.
	const cwd = /\.[a-z]+$/i.test(source_path) ? dirname(abs) : abs;
	const toplevel = await git(cwd, ['rev-parse', '--show-toplevel']);
	if (!toplevel) return null;
	const [commit, remote] = await Promise.all([
		git(toplevel, ['rev-parse', 'HEAD']),
		git(toplevel, ['remote', 'get-url', 'origin']),
	]);
	const url = normalize_github_url(remote);
	if (!url || !commit) return null;
	return {url, slug: new URL(url).pathname.slice(1), commit, subpath: relative(toplevel, abs)};
}

/**
 * Populate `source.repo` for every source (git-detected; left `undefined` for a
 * source with no GitHub origin, e.g. the local `svelte_styles` cache). Runs the
 * detections concurrently — a handful of cheap `git` calls at report-build time.
 */
export async function enrich_source_repos(sources: CorpusSource[]): Promise<void> {
	await Promise.all(
		sources.map(async (source) => {
			source.repo = (await detect_repo(source.path)) ?? undefined;
		}),
	);
}

/**
 * A `git clone <url> <dir>` triage line for an ABSENT corpus checkout, or `null`
 * when its URL isn't declared (the author's own `real`-tier repos — cloning those
 * isn't the fresh-machine story the suite/framework checkouts are). `<dir>` is the
 * checkout root the entry lives under (e.g. `../svelte`).
 */
export function clone_hint(source_path: string): string | null {
	const dir = checkout_prefix(source_path);
	const match = CLONE_URL_BY_PREFIX.find(([prefix]) => source_path.startsWith(prefix));
	return match ? `git clone ${match[1]} ${dir}` : null;
}
