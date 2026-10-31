/**
 * Publish the workspace's crates to crates.io — dry-run by default.
 *
 * The publishable set is `docsrs_crates.ts`'s (the members with a `[package.metadata.docs.rs]`
 * table), cross-checked against cargo's own reading of `publish`, and its order comes from
 * `cargo metadata`'s dependency edges, never a list kept here. All of them share the
 * workspace version.
 *
 * Each run, dry or not:
 *
 * 1. **Preflight**: the set, the version, the order; `--expect-tag v<version>` asserts the
 *    checkout's version is the tag's. A `--wetrun` also requires the checkout to BE the
 *    release: HEAD at the commit `v<version>` names and a clean tree (untracked files
 *    included) — a publish from any other commit would put source the release never had on
 *    crates.io under its version, for good. That subsumes `--expect-tag`. It is asked again
 *    before every `cargo publish`, so a `--wait` sleep cannot publish a checkout moved since.
 * 2. **Registry**: every crate's versions, from the crates.io API (`scripts/release/crates.ts`)
 *    — a lookup that fails is fatal, never read as "not published". Skipped, and said so,
 *    by the one run that reads none of it: a dry run with `--no-semver-checks`.
 * 3. **Public-API gate**: `cargo semver-checks` per crate, at the crate's docs.rs feature set
 *    named explicitly, against its published baseline. A crate with no published release has
 *    no baseline and is SKIPPED — said loudly, per crate and again in the summary, never
 *    counted as a pass. A version already live grades only with `--bump <level>` (the level
 *    the next release would take); `--no-semver-checks` skips the gate, as loudly.
 * 4. **Publish**: `cargo publish --workspace --locked`, which skips the `publish = false`
 *    members and orders the rest itself. First the overlay unpacks an earlier run left and
 *    the target directory's `package/` (`cargo metadata`'s `target_directory`, so
 *    `CARGO_TARGET_DIR` and `build.target-dir` count) are removed: at an unchanged version
 *    cargo reuses both, so a verification would build the sources of that earlier run, not
 *    this tree. The task's write fence names `target/package` and `$HOME/.cargo/registry/src`,
 *    so a target directory or cargo home elsewhere FAILS the run rather than going uncleared.
 *    - A dry run (the default) adds `--dry-run`: every crate is packaged and built alone
 *      from its tarball, in dependency order, against its siblings' tarballs.
 *    - `--wetrun` excludes every crate already live (a yanked version still holds its
 *      number), so a re-run publishes only what is missing, and with all of them live it is
 *      a no-op. crates.io rate-limits new crates (`NEW_CRATE_BURST`, then one per
 *      `NEW_CRATE_INTERVAL_MINUTES`), so a first publish of more crates stops part-way: the
 *      run names the time crates.io gives and exits 1, and re-running then goes on from the
 *      live set — or `--wait` sleeps until that time and goes on by itself. The registry is
 *      the resume state, so there is no sentinel file.
 *
 * Usage:
 *   deno task publish:crates                       # dry run
 *   deno task publish:crates --bump minor          # dry run, grading this tree as the next minor
 *   deno task publish:crates --wetrun [--wait]     # publish (the user's, by token, at the tag)
 *
 * Flags (no `--` separator: deno task forwards it literally and parseArgs rejects it):
 *   --wetrun             publish for real, from a clean checkout of the `v<version>` tag
 *   --expect-tag <tag>   fail unless the workspace version is the tag's (`v<version>`)
 *   --bump <level>       patch | minor | major — the level semver-checks grades an
 *                        already-live version at
 *   --no-semver-checks   skip the public-API gate (said loudly)
 *   --allow-dirty        package a dirty tree (dry run only)
 *   --wait               on a rate limit, sleep until crates.io's retry time and go on
 *
 * Not its job: the version bump and its `=x.y.z` sibling pins (`scripts/publish.ts`), the
 * changelog, tagging. The release workflow's `crates` job runs it as a dry run.
 */

import { readdirSync } from 'node:fs';
import { basename, join, relative } from 'node:path';
import { parseArgs } from 'node:util';

import { docs_rs_crates } from './docsrs_crates.ts';
import { command_capture, command_stream, command_tee } from './release/command.ts';
import {
	cargo_publish_args,
	cargo_publish_rate_limit,
	crate_version_state,
	crates_io_versions,
	crates_publish_order,
	crates_semver_plan,
	clear_package_state,
	is_overlay_parent,
	type MetadataPackage,
	NEW_CRATE_BURST,
	NEW_CRATE_INTERVAL_MINUTES,
	type PublishedVersions,
	semver_checks_args,
	type VersionState,
	wetrun_checkout_refusals
} from './release/crates.ts';
import {
	CARGO_PATH,
	cargo_workspace_version,
	is_bump_level,
	version_tag
} from './release/version.ts';

const { values: args } = parseArgs({
	options: {
		wetrun: { type: 'boolean' },
		'expect-tag': { type: 'string' },
		bump: { type: 'string' },
		'no-semver-checks': { type: 'boolean' },
		'allow-dirty': { type: 'boolean' },
		wait: { type: 'boolean' }
	}
});

const wetrun = args.wetrun === true;
// annotated, so a call narrows like a `throw` (control flow reads only annotated `never`s)
const fail: (message: string) => never = (message) => {
	console.error(`FAIL: ${message}`);
	Deno.exit(1);
};

if (args.bump !== undefined && !is_bump_level(args.bump)) {
	fail(`--bump must be patch, minor or major, not "${args.bump}"`);
}
const bump = is_bump_level(args.bump) ? args.bump : null;
if (wetrun && args['allow-dirty'])
	fail('--allow-dirty is for a dry run; a publish packages a clean tree');
if (!wetrun && args.wait) fail('--wait is for a --wetrun; a dry run hits no rate limit');

// Step 1: preflight

console.log(`=== publish:crates (${wetrun ? 'WETRUN — publishes to crates.io' : 'dry run'}) ===`);

const crates = docs_rs_crates();
if (crates.length === 0) fail('no workspace crate carries a [package.metadata.docs.rs] table');
const names = crates.map((c) => c.name);

const metadata = await command_capture('cargo', [
	'metadata',
	'--no-deps',
	'--format-version',
	'1',
	'--locked'
]);
if (!metadata.success) fail(`cargo metadata --locked failed:\n${metadata.err}`);
const { packages, target_directory } = JSON.parse(metadata.out) as {
	packages: Array<MetadataPackage>;
	target_directory: string;
};

// cargo's reading of the publishable set must be docsrs_crates.ts's (package:audit grades the
// same; asked again here because this is the step that publishes)
const cargo_publishable = packages.filter((p) => p.publish === null).map((p) => p.name);
const drift = [
	...cargo_publishable.filter((n) => !names.includes(n)),
	...names.filter((n) => !cargo_publishable.includes(n))
];
if (drift.length > 0) {
	fail(`cargo's publishable set and docsrs_crates.ts's disagree on ${drift.join(', ')}`);
}

const version = cargo_workspace_version(await Deno.readTextFile(CARGO_PATH));
if (version === null) fail(`no [workspace.package] version in ${CARGO_PATH}`);
const off_version = packages.filter((p) => names.includes(p.name) && p.version !== version);
if (off_version.length > 0) {
	fail(
		`every publishable crate takes the workspace version ${version}: ` +
			off_version.map((p) => `${p.name} is ${p.version}`).join(', ')
	);
}
if (args['expect-tag'] !== undefined && args['expect-tag'] !== version_tag(version)) {
	fail(
		`the workspace version v${version} does not match the tag ${args['expect-tag']} — ` +
			'the checkout and the tag disagree'
	);
}

/** Fail unless the checkout IS the release: HEAD at the commit `v<version>` names, and a
 * clean tree (see the module doc). Asked up front, to fail fast, and again immediately before
 * every `cargo publish` — a `--wait` sleep or the semver-checks run is time in which the
 * checkout can move. */
const assert_release_checkout = async (): Promise<void> => {
	const head = await command_capture('git', ['rev-parse', 'HEAD']);
	const tag_commit = await command_capture('git', [
		'rev-parse',
		'--verify',
		'--quiet',
		`${version_tag(version)}^{commit}`
	]);
	const status = await command_capture('git', ['status', '--porcelain']);
	if (!status.success) fail(`git status failed:\n${status.err}`);
	const refusals = wetrun_checkout_refusals({
		version,
		head: head.success ? head.out : null,
		tag_commit: tag_commit.success ? tag_commit.out : null,
		status_lines: status.out.split('\n').filter((line) => line !== '')
	});
	if (refusals.length > 0) fail(`refusing to publish:\n  ${refusals.join('\n  ')}`);
	console.log(`  checkout is ${version_tag(version)}, clean`);
};
if (wetrun) await assert_release_checkout();

const ordered = crates_publish_order(packages, names);
if (!ordered.ok) fail(ordered.error);
const { order } = ordered;
console.log(`  version ${version}; ${order.length} crates, in publish order: ${order.join(' → ')}`);

// Step 2: registry

// the one run that reads nothing from the registry: neither a gate nor a publish decision
const registry_needed = wetrun || !args['no-semver-checks'];

const registry = { fetch: (url: string, init: RequestInit) => fetch(url, init) };

/** Every crate's published versions; fails the run on any lookup error. */
const read_registry = async (): Promise<Map<string, PublishedVersions>> => {
	const answers = new Map<string, PublishedVersions>();
	for (const name of order) {
		const answer = await crates_io_versions(registry, name);
		if (!answer.ok) fail(`${answer.error}\n  no publish or gate decision is made without it`);
		answers.set(name, answer.versions);
	}
	return answers;
};

const states_of = (answers: Map<string, PublishedVersions>): Map<string, VersionState> =>
	new Map(order.map((name) => [name, crate_version_state(answers.get(name)!, version)]));

let published = new Map<string, PublishedVersions>();
if (!registry_needed) {
	console.log(
		'\n=== Registry (crates.io API) — SKIPPED: a dry run with --no-semver-checks reads nothing from it ==='
	);
} else {
	console.log('\n=== Registry (crates.io API) ===');
	published = await read_registry();
}
for (const [name, versions] of published) {
	const state = crate_version_state(versions, version);
	console.log(
		`  ${name}: ${versions === null ? 'not on crates.io' : `${versions.length} version(s) on crates.io`}` +
			`; v${version} ${state}`
	);
	if (state === 'yanked') {
		console.warn(
			`  WARN: ${name}@${version} is yanked — its number is taken, so it is not republished`
		);
	}
}

// Step 3: public-API gate

const semver_failed: Array<string> = [];
const semver_skipped: Array<string> = [];
if (args['no-semver-checks']) {
	console.warn('\n=== Public-API gate — SKIPPED (--no-semver-checks) ===');
	console.warn("  ⚠ cargo semver-checks did NOT run: no crate's public API was graded");
	semver_skipped.push(...order);
} else {
	console.log('\n=== Public-API gate (cargo semver-checks, docs.rs feature sets) ===');
	const tool = await command_capture('cargo', ['semver-checks', '--version']);
	if (!tool.success) {
		fail(
			'cargo semver-checks is not installed (`cargo install --locked cargo-semver-checks`); ' +
				'--no-semver-checks skips the gate'
		);
	}
	console.log(`  ${tool.out}`);
	const by_name = new Map(crates.map((c) => [c.name, c]));
	for (const name of order) {
		const plan = crates_semver_plan(published.get(name)!, version, bump);
		if (plan.kind === 'skip') {
			console.warn(`  ⚠ SKIP ${name}: ${plan.reason} — its public API is NOT graded`);
			semver_skipped.push(name);
			continue;
		}
		const semver_args = semver_checks_args(name, by_name.get(name)!, plan);
		console.log(`\n  cargo ${semver_args.join(' ')}`);
		const run = await command_stream('cargo', semver_args);
		if (!run.success) semver_failed.push(name);
	}
}

// Step 4: publish

/** Clear what an earlier packaging at this version left for cargo to reuse (see the module
 * doc), failing the run on anything it cannot remove — the write fence included. */
const clear_stale_packaging = (): void => {
	const home = Deno.env.get('CARGO_HOME') ?? join(Deno.env.get('HOME') ?? '', '.cargo');
	const registry_src = join(home, 'registry', 'src');
	const package_dir = join(target_directory, 'package');
	let removed: Array<string>;
	try {
		removed = clear_package_state(
			{
				read_dir: (path) => readdirSync(path),
				is_symlink: (path) => Deno.lstatSync(path).isSymlink,
				remove: (path, recursive) => Deno.removeSync(path, { recursive })
			},
			{ package_dir, registry_src, names: order, version }
		);
	} catch (error) {
		fail(
			`could not clear the stale packaging state (${package_dir}, ${registry_src}): ${String(error)}` +
				"\n  a target directory or cargo home outside the task's write fence is refused, not skipped"
		);
	}
	const overlays = removed.filter((path) => path.startsWith(registry_src));
	const parents = overlays.filter((path) => is_overlay_parent(basename(path))).length;
	console.log(
		removed.length === 0
			? '  no stale packaging state to clear'
			: '  cleared stale packaging state: ' +
					(removed.includes(package_dir)
						? `${relative(Deno.cwd(), package_dir) || package_dir}, `
						: '') +
					`${overlays.length - parents} overlay unpack(s) of v${version} and ${parents} emptied ` +
					`overlay dir(s) under ${registry_src}`
	);
};

let publish_ok = false;
if (!wetrun) {
	console.log('\n=== Publish — dry run (cargo publish --workspace --dry-run) ===');
	clear_stale_packaging();
	const publish_args = cargo_publish_args({
		dry_run: true,
		allow_dirty: args['allow-dirty'] === true,
		exclude: []
	});
	console.log(`  cargo ${publish_args.join(' ')}`);
	const run = await command_stream('cargo', publish_args);
	publish_ok = run.success;
	if (!run.success) console.error(`  FAIL: cargo publish --dry-run exited ${run.code}`);
} else if (semver_failed.length > 0) {
	console.error('\n=== Publish — REFUSED: the public-API gate failed ===');
} else {
	console.log('\n=== Publish (cargo publish --workspace) ===');
	let answers = published;
	for (;;) {
		const states = states_of(answers);
		const live = order.filter((n) => states.get(n) !== 'missing');
		const missing = order.filter((n) => states.get(n) === 'missing');
		if (missing.length === 0) {
			console.log(`  every crate is live at v${version} — nothing to publish`);
			publish_ok = true;
			break;
		}
		const new_crates = missing.filter((n) => answers.get(n) === null);
		if (new_crates.length > NEW_CRATE_BURST) {
			console.log(
				`  ${new_crates.length} new crate(s): crates.io admits ${NEW_CRATE_BURST}, then one per ` +
					`${NEW_CRATE_INTERVAL_MINUTES} minutes, so this publish stops part-way and resumes`
			);
		}
		clear_stale_packaging();
		const publish_args = cargo_publish_args({ dry_run: false, allow_dirty: false, exclude: live });
		await assert_release_checkout();
		console.log(`  cargo ${publish_args.join(' ')}`);
		const run = await command_tee('cargo', publish_args);
		if (run.success) {
			publish_ok = true;
			console.log(`  published ${missing.join(', ')} at v${version}`);
			break;
		}
		const limit = cargo_publish_rate_limit(run.err);
		if (!limit.limited) {
			console.error(`  FAIL: cargo publish exited ${run.code}`);
			break;
		}
		const at = limit.retry_at?.toUTCString() ?? 'the time crates.io names above';
		if (!args.wait || limit.retry_at === null) {
			console.error(
				`  STOPPED at crates.io's rate limit. Re-run \`deno task publish:crates --wetrun\` after ${at}: ` +
					'it publishes only the crates not yet live (or pass --wait to sleep through it).'
			);
			break;
		}
		const ms = Math.max(0, limit.retry_at.getTime() - Date.now()) + 30_000;
		console.log(`  rate limited — waiting until ${at} (+30 s), then going on from the live set`);
		await new Promise((resolve) => setTimeout(resolve, ms));
		answers = await read_registry();
	}
}

// Summary: the skips get the last word, so a gate that graded nothing is never a quiet pass

console.log('\n=== Summary ===');
console.log(
	`  publish: ${publish_ok ? (wetrun ? 'done' : 'dry run OK') : 'FAILED'}` +
		` (v${version}, ${order.length} crates)`
);
const graded = order.length - semver_skipped.length;
console.log(
	`  semver-checks: ${graded} graded, ${semver_failed.length} failed` +
		(semver_failed.length > 0 ? ` (${semver_failed.join(', ')})` : '')
);
if (semver_skipped.length > 0) {
	console.warn(
		`  ⚠ semver-checks SKIPPED ${semver_skipped.length} of ${order.length} crate(s): ` +
			`${semver_skipped.join(', ')} — their public API was NOT graded`
	);
}
if (!publish_ok || semver_failed.length > 0) Deno.exit(1);
