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
 *    this tree. The writes are granted beneath `target/package` and
 *    `$HOME/.cargo/registry/src` alone (`scripts/roots/package_state.ts`), so a target
 *    directory or cargo home elsewhere FAILS the run, naming the variable that moved it,
 *    rather than going uncleared.
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
 * Flags (through `deno task`, no `--` separator: it forwards one literally, and parseArgs
 * rejects what follows it; `npm run publish:crates -- <flags>` takes one):
 *   --wetrun             publish for real, from a clean checkout of the `v<version>` tag
 *   --expect-tag <tag>   fail unless the workspace version is the tag's (`v<version>`)
 *   --bump <level>       patch | minor | major — the level the next release takes, which
 *                        semver-checks grades an already-live version at, read as cargo
 *                        reads it before 1.0 (a `minor` bump of `0.y.z` may break)
 *   --no-semver-checks   skip the public-API gate (said loudly)
 *   --allow-dirty        package a dirty tree (dry run only)
 *   --wait               on a rate limit, sleep until crates.io's retry time and go on
 *
 * Recovery: crates.io never takes a version twice, so a broken release is yanked as a whole set
 * (the siblings pin each other `=x.y.z`) and fixed forward by the next version.
 *
 * Not its job: the version bump and its `=x.y.z` sibling pins (`scripts/publish.ts`), the
 * changelog, tagging. The release workflow's `crates` job runs it as a dry run.
 *
 * Its reach (`PublishCratesDeps`, built by `scripts/roots/publish_crates.ts`): the checkout
 * read-only, and cargo's and git's working directory; `cargo`; `git`, reading the checkout;
 * the crates.io API; and the cleanup's two directories. Cargo's semver-checks and publish
 * runs keep the terminal (the publish of a `--wetrun` shows stderr as it runs and keeps a copy
 * to read a rate limit from).
 *
 * @module
 */

import { basename, join, relative } from 'node:path';
import { parseArgs } from 'node:util';

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram, SpawnResult } from '@fuzdev/gro/spawn.ts';

import { docs_rs_crates } from './docsrs_crates.ts';
import { signal_exit_code } from './exit_code.ts';
import {
	cargo_publish_args,
	cargo_publish_rate_limit,
	clear_package_state,
	crate_version_state,
	type CratesIoDeps,
	crates_io_versions,
	crates_publish_order,
	crates_semver_plan,
	is_overlay_parent,
	type MetadataPackage,
	NEW_CRATE_BURST,
	NEW_CRATE_INTERVAL_MINUTES,
	type PackageStateDeps,
	type PublishedVersions,
	semver_checks_args,
	type VersionState,
	wetrun_checkout_refusals
} from './release/crates.ts';
import type { Result } from './release/result.ts';
import {
	CARGO_PATH,
	cargo_workspace_version,
	is_bump_level,
	version_tag
} from './release/version.ts';

/** What `run_publish_crates` reaches. */
export interface PublishCratesDeps {
	/** The checkout, read for its manifests, and cargo's and git's working directory. */
	checkout: FsReadDir;
	/** `cargo`, or `null` when there's none, which each call reports as not found. */
	cargo: SpawnProgram | null;
	/** `git`, reading the checkout, or `null` when there's none. */
	git: SpawnProgram | null;
	/** The crates.io API. */
	registry: CratesIoDeps;
	/** `$CARGO_HOME/registry/src`, as the output names it. */
	registry_src_path: string;
	/** The cleanup's two directories, opened for each run of it, from cargo's
	 * `target_directory`. */
	open_package_state: (target_directory: string) => Promise<PackageStateDeps>;
	/** Writes a line to stdout. */
	print: (line: string) => void;
	/** Writes a line to stderr. */
	print_error: (line: string) => void;
	/** Writes a child's stderr bytes to stderr, as it wrote them. */
	write_error: (bytes: Uint8Array) => Promise<unknown>;
	sleep: (ms: number) => Promise<void>;
	/** Milliseconds since the epoch. */
	now: () => number;
}

/** A captured call: its outcome, stdout with trailing whitespace trimmed (porcelain status
 * lines open with a space), and stderr trimmed. A missing program is a failure whose `err`
 * names it. */
interface Captured {
	success: boolean;
	out: string;
	err: string;
}

/** A call's outcome and exit code, `128 + N` for a child signal `N` ended; a missing program
 * exits `127`. */
interface Ended {
	success: boolean;
	code: number;
}

const to_ended = (result: SpawnResult): Ended => {
	if (result.kind === 'exited') return { success: result.code === 0, code: result.code };
	if (result.kind === 'signaled') return { success: false, code: signal_exit_code(result.signal) };
	return {
		success: false,
		code: result.code ?? (result.signal === null ? 1 : signal_exit_code(result.signal))
	};
};

const capture = async (
	program: SpawnProgram | null,
	name: string,
	args: Array<string>,
	cwd: FsReadDir
): Promise<Captured> => {
	if (program === null) return { success: false, out: '', err: `${name}: command not found` };
	const result = await program.out(args, { cwd });
	return {
		success: to_ended(result).success,
		out: result.stdout.trimEnd(),
		err: result.stderr.trim()
	};
};

/**
 * Why a `--wetrun` may not publish from this checkout (`wetrun_checkout_refusals`), from
 * `git rev-parse HEAD`, `git rev-parse --verify --quiet v<version>^{commit}`, and
 * `git status --porcelain`, each read in the checkout.
 * @returns the refusals, empty when the checkout is the release; an error when `git status`
 *   fails
 */
export const release_checkout_refusals = async (
	deps: Pick<PublishCratesDeps, 'checkout' | 'git'>,
	version: string
): Promise<Result<{ refusals: Array<string> }>> => {
	const git = (args: Array<string>) => capture(deps.git, 'git', args, deps.checkout);
	const head = await git(['rev-parse', 'HEAD']);
	const tag_commit = await git([
		'rev-parse',
		'--verify',
		'--quiet',
		`${version_tag(version)}^{commit}`
	]);
	const status = await git(['status', '--porcelain']);
	if (!status.success) return { ok: false, error: `git status failed:\n${status.err}` };
	return {
		ok: true,
		refusals: wetrun_checkout_refusals({
			version,
			head: head.success ? head.out : null,
			tag_commit: tag_commit.success ? tag_commit.out : null,
			status_lines: status.out.split('\n').filter((line) => line !== '')
		})
	};
};

/** Thrown by `fail` once it has printed, so `run_publish_crates` exits 1. */
class PublishCratesFailure extends Error {}

/** The flags `run_publish_crates` takes (module doc). */
interface PublishCratesArgs {
	wetrun?: boolean;
	'expect-tag'?: string;
	bump?: string;
	'no-semver-checks'?: boolean;
	'allow-dirty'?: boolean;
	wait?: boolean;
}

/**
 * Publish the crates, or dry-run it (module doc).
 * @param argv - the flags
 * @returns the exit code: `1` when a preflight refuses, the publish or its dry run fails, or a
 *   crate fails the public-API gate, else `0`
 * @throws `TypeError` - for a flag `parseArgs` refuses
 */
export async function run_publish_crates(
	deps: PublishCratesDeps,
	argv: ReadonlyArray<string>
): Promise<number> {
	const { values: args } = parseArgs({
		args: [...argv],
		options: {
			wetrun: { type: 'boolean' },
			'expect-tag': { type: 'string' },
			bump: { type: 'string' },
			'no-semver-checks': { type: 'boolean' },
			'allow-dirty': { type: 'boolean' },
			wait: { type: 'boolean' }
		}
	});
	try {
		return await publish_crates(deps, args);
	} catch (error) {
		if (error instanceof PublishCratesFailure) return 1;
		throw error;
	}
}

async function publish_crates(deps: PublishCratesDeps, args: PublishCratesArgs): Promise<number> {
	const { checkout, print, print_error } = deps;
	const wetrun = args.wetrun === true;
	// annotated, so a call narrows like a `throw` (control flow reads only annotated `never`s)
	const fail: (message: string) => never = (message) => {
		print_error(`FAIL: ${message}`);
		throw new PublishCratesFailure(message);
	};

	/** Runs cargo with the terminal attached, stdin included. */
	const cargo_stream = async (cargo_args: Array<string>): Promise<Ended> => {
		if (deps.cargo === null) {
			print_error('cargo: command not found');
			return { success: false, code: 127 };
		}
		return to_ended(await deps.cargo.run(cargo_args, { cwd: checkout }));
	};
	/** Runs cargo with its stdout attached and its stderr shown as it runs, keeping a copy of
	 * stderr to read a registry's refusal from. No stdin: nothing here prompts. */
	const cargo_tee = async (cargo_args: Array<string>): Promise<Ended & { err: string }> => {
		if (deps.cargo === null) {
			print_error('cargo: command not found');
			return { success: false, code: 127, err: 'cargo: command not found' };
		}
		const child = await deps.cargo.start(cargo_args, {
			cwd: checkout,
			stdio: ['ignore', 'inherit', 'pipe']
		});
		const chunks: Array<string> = [];
		const decoder = new TextDecoder();
		for await (const chunk of child.stderr!) {
			await deps.write_error(chunk);
			chunks.push(decoder.decode(chunk, { stream: true }));
		}
		chunks.push(decoder.decode());
		return { ...to_ended(await child.closed), err: chunks.join('') };
	};

	if (args.bump !== undefined && !is_bump_level(args.bump)) {
		fail(`--bump must be patch, minor or major, not "${args.bump}"`);
	}
	const bump = is_bump_level(args.bump) ? args.bump : null;
	if (wetrun && args['allow-dirty'])
		fail('--allow-dirty is for a dry run; a publish packages a clean tree');
	if (!wetrun && args.wait) fail('--wait is for a --wetrun; a dry run hits no rate limit');

	// Step 1: preflight

	print(`=== publish:crates (${wetrun ? 'WETRUN — publishes to crates.io' : 'dry run'}) ===`);

	const crates = docs_rs_crates((path) => checkout.read_text_sync(path));
	if (crates.length === 0) fail('no workspace crate carries a [package.metadata.docs.rs] table');
	const names = crates.map((c) => c.name);

	const metadata = await capture(
		deps.cargo,
		'cargo',
		['metadata', '--no-deps', '--format-version', '1', '--locked'],
		checkout
	);
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

	const version = cargo_workspace_version(await checkout.read_text(CARGO_PATH));
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
		const checked = await release_checkout_refusals(deps, version);
		if (!checked.ok) fail(checked.error);
		if (checked.refusals.length > 0) {
			fail(`refusing to publish:\n  ${checked.refusals.join('\n  ')}`);
		}
		print(`  checkout is ${version_tag(version)}, clean`);
	};
	if (wetrun) await assert_release_checkout();

	const ordered = crates_publish_order(packages, names);
	if (!ordered.ok) fail(ordered.error);
	const { order } = ordered;
	print(
		`  version ${version}; ${order.length} crates, in dependency order (cargo orders its own ` +
			`uploads): ${order.join(' → ')}`
	);

	// Step 2: registry

	// the one run that reads nothing from the registry: neither a gate nor a publish decision
	const registry_needed = wetrun || !args['no-semver-checks'];

	/** Every crate's published versions; fails the run on any lookup error. */
	const read_registry = async (): Promise<Map<string, PublishedVersions>> => {
		const answers = new Map<string, PublishedVersions>();
		for (const name of order) {
			const answer = await crates_io_versions(deps.registry, name);
			if (!answer.ok) fail(`${answer.error}\n  no publish or gate decision is made without it`);
			answers.set(name, answer.versions);
		}
		return answers;
	};

	const states_of = (answers: Map<string, PublishedVersions>): Map<string, VersionState> =>
		new Map(order.map((name) => [name, crate_version_state(answers.get(name)!, version)]));

	let published = new Map<string, PublishedVersions>();
	if (!registry_needed) {
		print(
			'\n=== Registry (crates.io API) — SKIPPED: a dry run with --no-semver-checks reads nothing from it ==='
		);
	} else {
		print('\n=== Registry (crates.io API) ===');
		published = await read_registry();
	}
	for (const [name, versions] of published) {
		const state = crate_version_state(versions, version);
		print(
			`  ${name}: ${versions === null ? 'not on crates.io' : `${versions.length} version(s) on crates.io`}` +
				`; v${version} ${state}`
		);
		if (state === 'yanked') {
			print_error(
				`  WARN: ${name}@${version} is yanked — its number is taken, so it is not republished`
			);
		}
	}

	// Step 3: public-API gate

	const semver_failed: Array<string> = [];
	const semver_skipped: Array<string> = [];
	if (args['no-semver-checks']) {
		print_error('\n=== Public-API gate — SKIPPED (--no-semver-checks) ===');
		print_error("  ⚠ cargo semver-checks did NOT run: no crate's public API was graded");
		semver_skipped.push(...order);
	} else {
		print('\n=== Public-API gate (cargo semver-checks, docs.rs feature sets) ===');
		const tool = await capture(deps.cargo, 'cargo', ['semver-checks', '--version'], checkout);
		if (!tool.success) {
			fail(
				'cargo semver-checks is not installed (`cargo install --locked cargo-semver-checks`); ' +
					'--no-semver-checks skips the gate'
			);
		}
		print(`  ${tool.out}`);
		const by_name = new Map(crates.map((c) => [c.name, c]));
		for (const name of order) {
			const plan = crates_semver_plan(published.get(name)!, version, bump);
			if (plan.kind === 'skip') {
				print_error(`  ⚠ SKIP ${name}: ${plan.reason} — its public API is NOT graded`);
				semver_skipped.push(name);
				continue;
			}
			const semver_args = semver_checks_args(name, by_name.get(name)!, plan);
			print(`\n  cargo ${semver_args.join(' ')}`);
			const run = await cargo_stream(semver_args);
			if (!run.success) semver_failed.push(name);
		}
	}

	// Step 4: publish

	/** Clear what an earlier packaging at this version left for cargo to reuse (see the module
	 * doc), failing the run on anything it cannot remove — a directory outside the grant included. */
	const clear_stale_packaging = async (): Promise<void> => {
		const registry_src = deps.registry_src_path;
		const package_dir = join(target_directory, 'package');
		let removed: Array<string>;
		try {
			removed = await clear_package_state(await deps.open_package_state(target_directory), {
				package_dir,
				registry_src,
				names: order,
				version
			});
		} catch (error) {
			fail(
				`could not clear the stale packaging state (${package_dir}, ${registry_src}): ${String(error)}` +
					'\n  a target directory or cargo home outside its grant is refused, not skipped'
			);
		}
		const overlays = removed.filter((path) => path.startsWith(registry_src));
		const parents = overlays.filter((path) => is_overlay_parent(basename(path))).length;
		print(
			removed.length === 0
				? '  no stale packaging state to clear'
				: '  cleared stale packaging state: ' +
						(removed.includes(package_dir)
							? `${relative(checkout.path, package_dir) || package_dir}, `
							: '') +
						`${overlays.length - parents} overlay unpack(s) of v${version} and ${parents} emptied ` +
						`overlay dir(s) under ${registry_src}`
		);
	};

	let publish_ok = false;
	if (!wetrun) {
		print('\n=== Publish — dry run (cargo publish --workspace --dry-run) ===');
		await clear_stale_packaging();
		const publish_args = cargo_publish_args({
			dry_run: true,
			allow_dirty: args['allow-dirty'] === true,
			exclude: []
		});
		print(`  cargo ${publish_args.join(' ')}`);
		const run = await cargo_stream(publish_args);
		publish_ok = run.success;
		if (!run.success) print_error(`  FAIL: cargo publish --dry-run exited ${run.code}`);
	} else if (semver_failed.length > 0) {
		print_error('\n=== Publish — REFUSED: the public-API gate failed ===');
	} else {
		print('\n=== Publish (cargo publish --workspace) ===');
		let answers = published;
		for (;;) {
			const states = states_of(answers);
			const live = order.filter((n) => states.get(n) !== 'missing');
			const missing = order.filter((n) => states.get(n) === 'missing');
			if (missing.length === 0) {
				print(`  every crate is live at v${version} — nothing to publish`);
				publish_ok = true;
				break;
			}
			const new_crates = missing.filter((n) => answers.get(n) === null);
			if (new_crates.length > NEW_CRATE_BURST) {
				print(
					`  ${new_crates.length} new crate(s): crates.io admits ${NEW_CRATE_BURST}, then one per ` +
						`${NEW_CRATE_INTERVAL_MINUTES} minutes, so this publish stops part-way and resumes`
				);
			}
			await clear_stale_packaging();
			const publish_args = cargo_publish_args({
				dry_run: false,
				allow_dirty: false,
				exclude: live
			});
			await assert_release_checkout();
			print(`  cargo ${publish_args.join(' ')}`);
			const run = await cargo_tee(publish_args);
			if (run.success) {
				publish_ok = true;
				print(`  published ${missing.join(', ')} at v${version}`);
				break;
			}
			const limit = cargo_publish_rate_limit(run.err);
			if (!limit.limited) {
				print_error(`  FAIL: cargo publish exited ${run.code}`);
				break;
			}
			const at = limit.retry_at?.toUTCString() ?? 'the time crates.io names above';
			if (!args.wait || limit.retry_at === null) {
				print_error(
					`  STOPPED at crates.io's rate limit. Re-run \`deno task publish:crates --wetrun\` after ${at}: ` +
						'it publishes only the crates not yet live (or pass --wait to sleep through it).'
				);
				break;
			}
			const ms = Math.max(0, limit.retry_at.getTime() - deps.now()) + 30_000;
			print(`  rate limited — waiting until ${at} (+30 s), then going on from the live set`);
			await deps.sleep(ms);
			answers = await read_registry();
		}
	}

	// Summary: the skips get the last word, so a gate that graded nothing is never a quiet pass

	print('\n=== Summary ===');
	print(
		`  publish: ${publish_ok ? (wetrun ? 'done' : 'dry run OK') : 'FAILED'}` +
			` (v${version}, ${order.length} crates)`
	);
	const graded = order.length - semver_skipped.length;
	print(
		`  semver-checks: ${graded} graded, ${semver_failed.length} failed` +
			(semver_failed.length > 0 ? ` (${semver_failed.join(', ')})` : '')
	);
	if (semver_skipped.length > 0) {
		print_error(
			`  ⚠ semver-checks SKIPPED ${semver_skipped.length} of ${order.length} crate(s): ` +
				`${semver_skipped.join(', ')} — their public API was NOT graded`
		);
	}
	return !publish_ok || semver_failed.length > 0 ? 1 : 0;
}
