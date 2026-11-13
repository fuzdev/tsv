/**
 * Publish the staged N-API npm packages — the release workflow's publish step,
 * also runnable locally with `--dry-run` (`npm run publish:napi -- --dry-run`). A local
 * run stages only the HOST platform package, so it deliberately ends at
 * the partial-set refusal below: staging + the refusal logic proven, nothing
 * published. The full-set rehearsal is the release workflow's dispatch dry run.
 *
 * Reads `crates/tsv_napi/pkg/`: the loader (`pkg/napi/`) plus every platform
 * package its generated `optionalDependencies` name (the loader package.json
 * is the single source of truth for the expected set — no third copy of the
 * triple list here). Refuses to publish a partial set: a missing platform
 * artifact — the `tsv_napi.node` addon or the native `tsv` CLI binary beside
 * it — means a matrix job failed, and a loader pointing at unpublished
 * optionalDependencies would break installs.
 *
 * Also re-arms the CLI binaries' executable bit: actions/upload-artifact does
 * not preserve file modes (every downloaded file arrives 644), npm packs the
 * on-disk mode into the tarball, and npm only chmods `bin` entries at install
 * (the loader's bin.js — the platform binary is not a bin entry). Without the
 * chmod every published posix platform would EACCES on `npx tsv`. That mode is
 * the only write, and it lands beneath `crates/tsv_napi/pkg/` or not at all.
 *
 * Publish order is platform packages FIRST, the loader LAST, so the loader
 * never goes live before the binaries it resolves. Idempotent like
 * `scripts/publish.ts`: a version already on the registry is skipped
 * (re-running a partially-failed workflow publishes only what's missing).
 * `npm view` errors other than E404 (auth, network) fail loudly rather than
 * being read as "not published". A `--dry-run` skips that check and runs
 * `npm publish --dry-run` over every package, reading npm's already-published
 * refusal as a pass — the rehearsal must pack each package even when its
 * version is live, which a dispatch or cron dry run between releases always is.
 *
 * Its entry is `scripts/roots/publish_napi.ts`, whose module doc says why this module and
 * everything it imports stay on `node:*` and relative modules, never a package. Every path it
 * reads or sets a mode on is the checkout's, never the working directory's, and `npm` runs in
 * the checkout, so the package directories it names (`crates/tsv_napi/pkg/<triple>`) are the
 * ones checked.
 *
 * Usage: node scripts/roots/publish_napi.ts [--dry-run] [--provenance] [--expect-tag v<version>]
 *
 * `--provenance` is passed by the release workflow (needs the workflow's
 * `id-token: write`); `--expect-tag` asserts the staged version matches the
 * triggering tag before anything publishes.
 *
 * @module
 */

import { isAbsolute, join, relative, sep } from 'node:path';
import { parseArgs } from 'node:util';

import { cli_binary_name } from './napi_host.ts';
import type { CapturedRun } from './release/node_host.ts';

/** Where the staged packages sit, relative to the checkout: the loader in `napi/`, each
 * platform package in `<triple>/`. */
export const NAPI_PKG_ROOT = 'crates/tsv_napi/pkg';

/** What npm says when the version is already live — a dry run's pass, since it packs the
 * staged version without bumping (the same reading `scripts/publish.ts` takes). */
// Only the over-an-existing-version refusal: npm 11's other client-side one (a version below
// the registry's highest cannot implicitly take `latest`) also says "previously published
// version", and is a real failure.
export const ALREADY_PUBLISHED = /cannot publish over the previously published versions/i;

/** What `run_publish_napi` reaches. */
export interface PublishNapiDeps {
	/** The checkout's path: the staged packages beneath it, and where `npm` runs. */
	checkout: string;
	/** The host as Node names it: the CLI binary's mode is set everywhere but `win32`. */
	platform: string;
	read_text: (path: string) => Promise<string>;
	/** Whether the path, symlinks followed, is a regular file; never throws. */
	is_file: (path: string) => Promise<boolean>;
	set_mode: (path: string, mode: number) => Promise<void>;
	/** The file's mode bits, symlinks followed. */
	mode: (path: string) => Promise<number>;
	/** One `npm` run in the checkout, its output captured. */
	npm: (args: Array<string>) => Promise<CapturedRun>;
	/** Prints a line to stdout. */
	print: (line: string) => void;
	/** Prints a line to stderr. */
	print_error: (line: string) => void;
}

type StagedPackage = { name: string; version: string; [k: string]: unknown };

/** A refusal that ends the run with exit 1, its lines already printed. */
class PublishNapiExit extends Error {}

/**
 * Check the staged set, re-arm the CLI binaries' mode, and publish platforms then loader
 * (module doc).
 * @returns the exit code: 0 once every package is published, skipped, or (under `--dry-run`)
 *   packed; 1 for an unreadable package.json, a loader with no platforms, a tag the staged
 *   version disagrees with, a partial or mismatched set, a binary whose mode didn't take, an
 *   `npm view` error other than E404, or a failed `npm publish`
 * @throws `TypeError` - for a flag `parseArgs` refuses
 * @throws `Error` - for a platform package name that would put its binary outside
 *   `crates/tsv_napi/pkg/`, before its mode is set
 */
export async function run_publish_napi(
	deps: PublishNapiDeps,
	argv: Array<string>
): Promise<number> {
	const { values: args } = parseArgs({
		args: argv,
		options: {
			'dry-run': { type: 'boolean' },
			provenance: { type: 'boolean' },
			'expect-tag': { type: 'string' }
		}
	});
	try {
		await publish(deps, args);
		return 0;
	} catch (error) {
		if (error instanceof PublishNapiExit) return 1;
		throw error;
	}
}

const publish = async (
	deps: PublishNapiDeps,
	args: { 'dry-run'?: boolean; provenance?: boolean; 'expect-tag'?: string }
): Promise<void> => {
	const at = (path: string): string => join(deps.checkout, path);
	const fail = (...lines: Array<string>): never => {
		for (const line of lines) deps.print_error(line);
		throw new PublishNapiExit();
	};

	const read_pkg = async (dir: string): Promise<StagedPackage> => {
		try {
			return JSON.parse(await deps.read_text(at(`${dir}/package.json`)));
		} catch (e) {
			return fail(`FAIL: cannot read ${dir}/package.json — is the set staged?`, String(e));
		}
	};

	const loader = await read_pkg(`${NAPI_PKG_ROOT}/napi`);
	const version = loader.version;
	const optional = loader.optionalDependencies as Record<string, string> | undefined;
	if (!optional || Object.keys(optional).length === 0) {
		fail('FAIL: loader package.json has no optionalDependencies — stale staging?');
	}

	if (args['expect-tag'] && args['expect-tag'] !== `v${version}`) {
		fail(
			`FAIL: staged version v${version} does not match the triggering tag ${args['expect-tag']} — ` +
				`the checkout and the staged artifacts disagree`
		);
	}

	// Completeness: every platform package the loader pins must be staged, at the
	// loader's own version, with both its binaries present. Partial sets never
	// publish.
	const platform_dirs: Array<{ name: string; dir: string }> = [];
	let incomplete = false;
	for (const [name, pin] of Object.entries(optional!)) {
		const triple = name.replace('@fuzdev/tsv-', '');
		const dir = `${NAPI_PKG_ROOT}/${triple}`;
		const cli_binary = cli_binary_name(triple);
		let missing = false;
		for (const file of ['tsv_napi.node', cli_binary]) {
			if (!(await deps.is_file(at(`${dir}/${file}`)))) {
				deps.print_error(`MISSING: ${dir}/${file} (${name}) — did its matrix job fail?`);
				missing = true;
			}
		}
		if (missing) {
			incomplete = true;
			continue;
		}
		const pkg = await read_pkg(dir);
		if (pkg.name !== name || pkg.version !== version || pin !== version) {
			deps.print_error(
				`MISMATCH: ${dir} stages ${pkg.name}@${pkg.version}, loader pins ${name}@${pin}, ` +
					`loader version ${version}`
			);
			incomplete = true;
			continue;
		}
		// Re-arm the x-bit lost in artifact transport (see the module doc), then
		// VERIFY it took — npm packs whatever mode is on disk, so a transport or
		// filesystem change that resists the chmod must fail the publish here,
		// not surface as a broken `npx tsv` on the registry. A posix binary
		// staged on a Windows host has no mode to fix, and none of the publish
		// paths run there (CI publishes from ubuntu).
		if (cli_binary === 'tsv' && deps.platform !== 'win32') {
			const binary = at(`${dir}/${cli_binary}`);
			// lexical, as the write fence on `crates/tsv_napi/pkg` was: a name whose triple climbs
			// out of it never reaches the chmod
			const beneath = relative(at(NAPI_PKG_ROOT), binary);
			if (isAbsolute(beneath) || beneath === '..' || beneath.startsWith(`..${sep}`)) {
				throw new Error(`refusing to set a mode outside ${NAPI_PKG_ROOT}: ${dir}/${cli_binary}`);
			}
			await deps.set_mode(binary, 0o755);
			const mode = await deps.mode(binary);
			if ((mode & 0o111) === 0) {
				deps.print_error(`NOT EXECUTABLE: ${dir}/${cli_binary} — chmod 755 did not take`);
				incomplete = true;
				continue;
			}
		}
		platform_dirs.push({ name, dir });
	}
	if (incomplete) fail('FAIL: refusing to publish a partial set');

	const npm = async (npm_args: Array<string>): Promise<CapturedRun> => {
		const run = await deps.npm(npm_args);
		return { code: run.code, stdout: run.stdout.trim(), stderr: run.stderr.trim() };
	};

	/** Whether `name@version` is already on the registry; errors other than E404 are fatal.
	 * `--json=false` because npm's `json` setting (an npmrc, `npm_config_json`) would print the
	 * version JSON-quoted. An exit-0 answer with unexpected stdout reads as NOT published — the
	 * safe degradation, since `npm publish` itself fails loudly on a real conflict, where
	 * aborting here would kill a release on an npm output quirk. */
	const already_published = async (name: string): Promise<boolean> => {
		const res = await npm(['view', `${name}@${version}`, 'version', '--json=false']);
		if (res.code === 0) return res.stdout === version;
		if (res.stderr.includes('E404')) return false;
		return fail(`FAIL: npm view ${name}@${version} errored (not E404):`, res.stderr || res.stdout);
	};

	// Platforms first, loader last — the loader must never precede its binaries.
	const to_publish = [...platform_dirs, { name: loader.name, dir: `${NAPI_PKG_ROOT}/napi` }];
	let published = 0;
	let skipped = 0;
	for (const { name, dir } of to_publish) {
		// A dry run checks nothing against the registry first: it exists to run `npm publish
		// --dry-run` over every package (the rehearsal release.yml's dispatch and weekly
		// cron promise), and the staged version is usually already live there — skipping
		// those would rehearse nothing.
		if (!args['dry-run'] && (await already_published(name))) {
			deps.print(`skip: ${name}@${version} already on the registry`);
			skipped++;
			continue;
		}
		const publish_args = [
			'publish',
			dir,
			'--access',
			'public',
			...(args.provenance ? ['--provenance'] : []),
			...(args['dry-run'] ? ['--dry-run'] : [])
		];
		deps.print(`npm ${publish_args.join(' ')}`);
		const res = await npm(publish_args);
		if (res.code !== 0 && args['dry-run'] && ALREADY_PUBLISHED.test(res.stderr)) {
			deps.print(`PASS: ${name} packs cleanly (v${version} already on the registry)`);
			skipped++;
			continue;
		}
		if (res.code !== 0) {
			fail(`FAIL: npm publish ${name}@${version} exited ${res.code}:`, res.stderr || res.stdout);
		}
		published++;
	}
	deps.print(
		args['dry-run']
			? `DRY-RUN: ${published + skipped} of ${to_publish.length} packed cleanly at v${version} ` +
					`(${published} would publish, ${skipped} already on the registry)`
			: `published ${published}, skipped ${skipped} (of ${to_publish.length}) at v${version}`
	);
};
