/**
 * The crates.io questions a release asks, and the decisions `scripts/publish_crates.ts` makes
 * from their answers: which crates publish in which order, which are already live, what
 * `cargo semver-checks` grades each one against, and when crates.io's rate limit lets a stopped
 * publish go on. The registry is reached only through an injected `fetch`, so every decision
 * here is unit-tested offline (`crates_test.ts`).
 *
 * @module
 */

import { type Result } from './command.ts';
import { type BumpLevel, version_parse, version_tag } from './version.ts';

/** The crates.io web API. Liveness is read here, not from the sparse index: the index is
 * served through a CDN that can lag a publish, while the API answers from the database. */
export const CRATES_IO_API = 'https://crates.io/api/v1/crates';

/** crates.io refuses API requests that carry no identifying `User-Agent`. */
export const CRATES_IO_USER_AGENT = 'tsv-release (https://github.com/fuzdev/tsv)';

/** crates.io's per-account limit on publishing NEW crates: a burst, then one per interval.
 * A first publish of more crates than the burst stops part-way and resumes later. New
 * VERSIONS of existing crates have a far larger burst that a release stays inside. */
export const NEW_CRATE_BURST = 5;
export const NEW_CRATE_INTERVAL_MINUTES = 10;

/** The capability the registry questions need: `fetch`, injected so tests answer offline. */
export interface CratesIoDeps {
	fetch: (url: string, init: RequestInit) => Promise<Response>;
}

/** One published version of a crate. */
export interface PublishedVersion {
	num: string;
	yanked: boolean;
}

/** Every version crates.io holds for one crate; `null` when the crate does not exist there. */
export type PublishedVersions = Array<PublishedVersion> | null;

/**
 * Every version crates.io holds for `name`, or `null` when no such crate exists (a 404). Any
 * other answer — a network failure, a 5xx, a 403 for a missing `User-Agent`, a body that is
 * not the expected shape — is an error: no publish or gate decision may be made without the
 * answer.
 */
export const crates_io_versions = async (
	deps: CratesIoDeps,
	name: string
): Promise<Result<{ versions: PublishedVersions }>> => {
	const url = `${CRATES_IO_API}/${encodeURIComponent(name)}`;
	let response: Response;
	try {
		response = await deps.fetch(url, {
			headers: { 'User-Agent': CRATES_IO_USER_AGENT, Accept: 'application/json' }
		});
	} catch (error) {
		return { ok: false, error: `GET ${url} failed: ${String(error)}` };
	}
	if (response.status === 404) {
		await response.body?.cancel();
		return { ok: true, versions: null };
	}
	const text = await response.text();
	if (response.status !== 200) {
		return { ok: false, error: `GET ${url} answered ${response.status}: ${text.slice(0, 500)}` };
	}
	let body: unknown;
	try {
		body = JSON.parse(text);
	} catch {
		return { ok: false, error: `GET ${url} answered 200 with a body that is not JSON` };
	}
	const versions = (body as { versions?: unknown }).versions;
	if (
		!Array.isArray(versions) ||
		!versions.every(
			(v) =>
				typeof v === 'object' &&
				v !== null &&
				typeof (v as PublishedVersion).num === 'string' &&
				typeof (v as PublishedVersion).yanked === 'boolean'
		)
	) {
		return { ok: false, error: `GET ${url} answered 200 without a versions list` };
	}
	return {
		ok: true,
		versions: versions.map((v: PublishedVersion) => ({ num: v.num, yanked: v.yanked }))
	};
};

/** Where `version` stands on crates.io: never published, live, or published and yanked. A
 * yanked version still occupies its number — crates.io never takes the same version twice —
 * so a publish treats it as live. */
export type VersionState = 'missing' | 'live' | 'yanked';

export const crate_version_state = (versions: PublishedVersions, version: string): VersionState => {
	const found = versions?.find((v) => v.num === version);
	if (!found) return 'missing';
	return found.yanked ? 'yanked' : 'live';
};

/** Compare two `<major>.<minor>.<patch>` versions; null when either is not one. */
export const version_compare = (a: string, b: string): number | null => {
	const va = version_parse(a);
	const vb = version_parse(b);
	if (!va || !vb) return null;
	return va.major - vb.major || va.minor - vb.minor || va.patch - vb.patch;
};

/** A crate as `cargo metadata --no-deps` describes it, cut to what the order reads. */
export interface MetadataPackage {
	name: string;
	version: string;
	publish: Array<string> | null;
	dependencies: Array<{ name: string; kind: string | null; path?: string }>;
}

/**
 * The publishable crates in an order cargo can publish them: every crate after each crate it
 * depends on through a normal or build edge (dev edges ship nothing a consumer resolves).
 * Ties keep `names`' own order, so the answer is stable. An error names a crate that
 * metadata does not describe, or a cycle.
 */
export const crates_publish_order = (
	packages: ReadonlyArray<MetadataPackage>,
	names: ReadonlyArray<string>
): Result<{ order: Array<string> }> => {
	const set = new Set(names);
	const by_name = new Map(packages.map((p) => [p.name, p]));
	const deps = new Map<string, Set<string>>();
	for (const name of names) {
		const pkg = by_name.get(name);
		if (!pkg) return { ok: false, error: `${name} is not in cargo metadata` };
		deps.set(
			name,
			new Set(
				pkg.dependencies
					.filter((d) => d.kind !== 'dev' && set.has(d.name) && d.name !== name)
					.map((d) => d.name)
			)
		);
	}
	const order: Array<string> = [];
	const placed = new Set<string>();
	while (order.length < names.length) {
		const next = names.find((n) => !placed.has(n) && [...deps.get(n)!].every((d) => placed.has(d)));
		if (next === undefined) {
			const stuck = names.filter((n) => !placed.has(n));
			return { ok: false, error: `dependency cycle among ${stuck.join(', ')}` };
		}
		order.push(next);
		placed.add(next);
	}
	return { ok: true, order };
};

/** What `cargo semver-checks` grades one crate against, or why it cannot. */
export type SemverPlan =
	| { kind: 'skip'; reason: string }
	| { kind: 'check'; baseline: string; release_type: BumpLevel | null };

/**
 * The semver gate's plan for one crate at `version`. The baseline is a published release on
 * crates.io — there is no committed snapshot — so a crate with none is SKIPPED, with a reason
 * the caller must print, never graded as a pass.
 *
 * - Not yet released at `version` (the release case — the tag's version, after the bump): the
 *   baseline is the highest published release below it, and the release type is left to
 *   `cargo semver-checks` to derive from the two version numbers. A `bump` is not needed.
 * - Already released at `version` (a rehearsal between releases): there is no release to
 *   grade unless `bump` names the level the next one would take; the tree is then graded
 *   against `version` itself at that level.
 */
export const crates_semver_plan = (
	versions: PublishedVersions,
	version: string,
	bump: BumpLevel | null
): SemverPlan => {
	const releases = (versions ?? []).filter((v) => !v.yanked && version_parse(v.num) !== null);
	if (releases.length === 0) {
		return {
			kind: 'skip',
			reason:
				versions === null
					? 'not on crates.io yet, so no published baseline'
					: 'no unyanked release on crates.io, so no published baseline'
		};
	}
	if (releases.some((v) => v.num === version)) {
		if (bump === null) {
			return {
				kind: 'skip',
				reason: `v${version} is already on crates.io, so there is no release to grade (--bump <level> grades this tree as the next one)`
			};
		}
		return { kind: 'check', baseline: version, release_type: bump };
	}
	const below = releases
		.map((v) => v.num)
		.filter((num) => (version_compare(num, version) ?? 0) < 0)
		.sort((a, b) => version_compare(b, a)!);
	if (below.length === 0) {
		return {
			kind: 'skip',
			reason: `every release on crates.io is above v${version}, so no baseline precedes it`
		};
	}
	return { kind: 'check', baseline: below[0]!, release_type: null };
};

/** The docs.rs settings that name a crate's public feature set (`docsrs_crates.ts`). */
export interface FeatureSet {
	features: ReadonlyArray<string>;
	all_features: boolean;
	no_default_features: boolean;
}

/**
 * `cargo semver-checks` arguments for one crate: its baseline, and its features stated
 * EXPLICITLY as the docs.rs feature set — the public surface a consumer reads, never the
 * tool's own heuristic (every feature but a few name patterns), so the graded surface cannot
 * drift from what docs.rs renders when a feature is added or renamed.
 */
export const semver_checks_args = (
	name: string,
	feature_set: FeatureSet,
	plan: Extract<SemverPlan, { kind: 'check' }>
): Array<string> => [
	'semver-checks',
	'--package',
	name,
	'--baseline-version',
	plan.baseline,
	...(feature_set.all_features
		? ['--all-features']
		: feature_set.no_default_features
			? ['--only-explicit-features']
			: ['--default-features']),
	...feature_set.features.flatMap((f) => ['--features', f]),
	...(plan.release_type === null ? [] : ['--release-type', plan.release_type])
];

/** `cargo publish` arguments over the workspace (cargo skips its `publish = false` members and
 * orders the rest itself), leaving out the crates already live. */
export const cargo_publish_args = (options: {
	dry_run: boolean;
	allow_dirty: boolean;
	exclude: ReadonlyArray<string>;
}): Array<string> => [
	'publish',
	'--workspace',
	'--locked',
	...(options.dry_run ? ['--dry-run'] : []),
	...(options.allow_dirty ? ['--allow-dirty'] : []),
	...options.exclude.flatMap((name) => ['--exclude', name])
];

/**
 * Whether a failed `cargo publish` hit crates.io's rate limit, and when it may go on. Cargo
 * reports the registry's answer as `status 429 Too Many Requests`, whose body names the time
 * (`Please try again after <HTTP date>`); `retry_at` is null when a limited answer names none.
 */
export const cargo_publish_rate_limit = (
	stderr: string
): { limited: false } | { limited: true; retry_at: Date | null } => {
	if (!/status 429\b|too many requests/i.test(stderr)) return { limited: false };
	const after =
		/try again after ([A-Za-z]{3}, \d{1,2} [A-Za-z]{3} \d{4} \d{2}:\d{2}:\d{2} GMT)/.exec(stderr);
	const retry_at = after ? new Date(after[1]!) : null;
	return {
		limited: true,
		retry_at: retry_at !== null && !Number.isNaN(retry_at.getTime()) ? retry_at : null
	};
};

/**
 * Whether `child` of the `parent` directory under `$CARGO_HOME/registry/src/` is a local
 * overlay's unpack of one of this release's crates. `cargo publish --workspace` packages each
 * crate, then unpacks the siblings' tarballs into such a directory (named `-<hash>`, the
 * overlay registry having no host name) to verify the dependents against them — and at an
 * unchanged version it REUSES an unpack left by an earlier run, so a fix made since is not
 * the source verified. Only these directories are cleared before a publish.
 */
export const is_overlay_unpack = (
	parent: string,
	child: string,
	names: ReadonlyArray<string>,
	version: string
): boolean => is_overlay_parent(parent) && names.some((n) => child === `${n}-${version}`);

/** Whether a directory under `$CARGO_HOME/registry/src/` is a local overlay's (`-<hash>`),
 * the only kind of parent the cleanup may touch. */
export const is_overlay_parent = (name: string): boolean => /^-[0-9a-f]{16}$/.test(name);

/** The filesystem the packaging cleanup needs, injected so the tests run it over a fake. A
 * missing path throws `Deno.errors.NotFound`; anything else that throws (a permission fence
 * included) is fatal to the caller. */
export interface PackageStateDeps {
	read_dir: (path: string) => Array<string>;
	/** Whether `path` itself is a symlink (an lstat: never followed). */
	is_symlink: (path: string) => boolean;
	/** Remove `path`: a whole tree when `recursive`, else only an empty directory. */
	remove: (path: string, recursive: boolean) => void;
}

/**
 * Remove what an earlier packaging at this version left for cargo to reuse: the target
 * directory's `package/` (from `cargo metadata`'s `target_directory`, so `CARGO_TARGET_DIR`
 * and `build.target-dir` are honored), and this release's overlay unpacks under
 * `registry_src` (`is_overlay_unpack`). An overlay parent left empty by the removal goes too,
 * by a NON-recursive remove, so it can never take content with it. A symlinked parent is
 * skipped, never followed. Returns every path
 * removed; throws on any failure but a path already gone.
 */
export const clear_package_state = (
	deps: PackageStateDeps,
	options: {
		package_dir: string;
		registry_src: string;
		names: ReadonlyArray<string>;
		version: string;
	}
): Array<string> => {
	const removed: Array<string> = [];
	const remove = (path: string, recursive: boolean): void => {
		try {
			deps.remove(path, recursive);
			removed.push(path);
		} catch (error) {
			if (!(error instanceof Deno.errors.NotFound)) throw error;
		}
	};
	const list = (dir: string): Array<string> | null => {
		try {
			return deps.read_dir(dir);
		} catch (error) {
			if (error instanceof Deno.errors.NotFound) return null;
			throw error;
		}
	};
	remove(options.package_dir, true);
	for (const parent of list(options.registry_src) ?? []) {
		if (!is_overlay_parent(parent)) continue;
		const parent_path = `${options.registry_src}/${parent}`;
		// a symlinked parent leads outside the cargo home, past a write fence that checks the
		// path as written: cargo never makes one, so it is no unpack of ours
		if (deps.is_symlink(parent_path)) continue;
		const children = list(parent_path) ?? [];
		const stale = children.filter((child) =>
			is_overlay_unpack(parent, child, options.names, options.version)
		);
		if (stale.length === 0) continue;
		for (const child of stale) remove(`${parent_path}/${child}`, true);
		if ((list(parent_path) ?? ['?']).length === 0) remove(parent_path, false);
	}
	return removed;
};

/** Why a `--wetrun` may not publish from this checkout, empty when it may: the crates must be
 * the source of the release they are versioned as, so HEAD must be the commit the version's
 * tag names (a tag missing locally refuses too — fetch it), and the tree must be clean
 * (untracked files included, since cargo packages them). */
export const wetrun_checkout_refusals = (checkout: {
	version: string;
	head: string | null;
	tag_commit: string | null;
	status_lines: ReadonlyArray<string>;
}): Array<string> => {
	const tag = version_tag(checkout.version);
	const refusals: Array<string> = [];
	if (checkout.head === null) refusals.push('cannot read HEAD (git rev-parse HEAD failed)');
	if (checkout.tag_commit === null) {
		refusals.push(`no local tag ${tag} — the version is published from its tag (git fetch --tags)`);
	} else if (checkout.head !== null && checkout.head !== checkout.tag_commit) {
		refusals.push(
			`HEAD ${checkout.head.slice(0, 12)} is not ${tag} (${checkout.tag_commit.slice(0, 12)}) — ` +
				`check out the tag: crates published at v${checkout.version} must be its source`
		);
	}
	if (checkout.status_lines.length > 0) {
		refusals.push(
			`the tree is not clean (${checkout.status_lines.length} path(s) in git status), ` +
				`so it is not ${tag}'s source:\n    ${checkout.status_lines.join('\n    ')}`
		);
	}
	return refusals;
};
