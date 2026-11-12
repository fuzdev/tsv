/**
 * Collect the GitHub Release assets for one version: the native `tsv` CLI
 * binary of every `@fuzdev/tsv-<triple>` platform package, a `SHA512SUMS`
 * file (`sha512sum -c` format) over those binaries, and the third-party
 * `NOTICE` the binaries' license terms require beside them (`THIRD_PARTY_NOTICE`,
 * copied from the checkout rather than from the registry: in the workflow the checkout
 * is the tag's own, so it is the file the packages ship; a local run against an older
 * version takes the current one).
 * The release workflow's `release` job runs this after the npm publish and
 * uploads the directory.
 *
 * The binaries come FROM THE REGISTRY, not from the workflow's build artifacts:
 * `npm pack` of each platform package the published loader names in its
 * `optionalDependencies`, extracting `package/tsv` (`tsv.exe` on Windows). So
 * the asset is byte-identical to what `npm install @fuzdev/tsv` serves by
 * construction — including on a recovery re-run, where the matrix rebuilds
 * fresh (non-identical) binaries while the publish step skips the versions
 * already live; artifact-sourced assets would silently diverge from npm there.
 * Reading the loader's dependency set from the registry also keeps this
 * script free of a platform list of its own (the loader package.json is the
 * source of truth; a platform missing there is a publish bug, not a release
 * asset to skip).
 *
 * The registry reads retry for a bounded window: in the workflow this runs
 * seconds after the publish job, and a just-published version can lag the
 * registry's read path (a fresh packument, more so a first-ever package) — the
 * one failure that could redden the release job after npm is already public.
 * A genuinely unpublished version still fails, just after the window.
 *
 * Asset names carry the npm triple (`tsv-linux-x64-gnu`, `tsv-win32-x64.exe`),
 * so an asset and the platform package that ships it share one name. Bare
 * binaries rather than archives: the download is one `curl` + `chmod +x`, the
 * checksum file covers integrity, and the workflow attests every asset's
 * provenance (`actions/attest-build-provenance`; `gh attestation verify <file>
 * -R fuzdev/tsv`).
 *
 * What the registry answers can't steer a write: a platform package's triple is a lowercase
 * name with no path in it, so each asset lands directly in `--out`; the tarball `npm pack`
 * reports is a file name in the temp directory; `tar` extracts the one binary into that
 * temp directory, and the copy refuses an extracted binary that is no regular file (a
 * symlink, say). Writes go to `--out` and the temp directory alone, which is removed
 * whether or not the run succeeds.
 *
 * Its entry is `scripts/roots/release_assets.ts`, whose module doc says why this module and
 * everything it imports stay on `node:*` and relative modules, never a package. `--out` is
 * read against the working directory; the NOTICE is the checkout's.
 *
 * Usage: node scripts/roots/release_assets.ts v<version> --out <dir>
 *        npm run release:assets -- v0.3.0 --out /tmp/tsv_assets
 *
 * Re-runnable locally against any published version, which is also how a
 * release's `SHA512SUMS` can be re-derived from npm and compared.
 *
 * @module
 */

import { join, resolve } from 'node:path';
import { parseArgs } from 'node:util';

import { cli_binary_name, release_asset_name } from './napi_host.ts';
import { THIRD_PARTY_NOTICE } from './npm_metadata.ts';
import type { CapturedRun } from './release/node_host.ts';
import { version_from_tag } from './release/version.ts';

/** The published loader, whose `optionalDependencies` name the platform packages. */
export const LOADER = '@fuzdev/tsv';

/** How long a registry read is given to see a just-published version:
 * `REGISTRY_ATTEMPTS` tries, `REGISTRY_RETRY_MS` apart (about ten minutes of
 * waiting in total). A first-ever package's packument has been seen to lag the
 * read path by minutes, so the window is sized for that case; the workflow's job
 * timeout must stay above it. */
export const REGISTRY_ATTEMPTS = 30;
export const REGISTRY_RETRY_MS = 20_000;

/** The prefix of the temp directory the tarballs are packed and extracted in. */
export const PACK_DIR_PREFIX = 'tsv_release_assets_';

/** A platform package's name, its triple captured: lowercase words joined by `-`, so the
 * triple holds no path. */
const PLATFORM_PACKAGE = /^@fuzdev\/tsv-([a-z0-9]+(?:-[a-z0-9]+)*)$/;

/** `tar`'s arguments for taking `member` alone out of the gzipped `tarball` into `dir`. */
export const tar_extract_args = (tarball: string, dir: string, member: string): Array<string> => [
	'-xzf',
	tarball,
	'-C',
	dir,
	member
];

/** The file operations `run_release_assets` takes, by path. */
export interface ReleaseAssetsFs {
	/** Creates the directory and any missing parents. */
	make_dir: (path: string) => Promise<void>;
	/** A new directory beneath the system's temp directory whose name starts with `prefix`.
	 * @returns its absolute path */
	make_temp_dir: (prefix: string) => Promise<string>;
	/** Removes the directory and everything beneath it. */
	remove_dir: (path: string) => Promise<void>;
	/** Copies the file, keeping its mode. */
	copy_file: (from: string, to: string) => Promise<void>;
	/** Whether the path itself, a symlink not followed, is a regular file; never throws. */
	is_regular_file: (path: string) => Promise<boolean>;
	/** The file's size in bytes. */
	size: (path: string) => Promise<number>;
	read_bytes: (path: string) => Promise<Uint8Array<ArrayBuffer>>;
	write_text: (path: string, text: string) => Promise<void>;
}

/** What `run_release_assets` reaches. */
export interface ReleaseAssetsDeps {
	/** The checkout's path, where the NOTICE is read. */
	checkout: string;
	/** The working directory, which a relative `--out` is read against. */
	cwd: string;
	/** One `npm` run, its output captured. */
	npm: (args: Array<string>) => Promise<CapturedRun>;
	/** One `tar` run, its output captured. */
	tar: (args: Array<string>) => Promise<CapturedRun>;
	/** Waits `ms` milliseconds: the pause between registry attempts. */
	sleep: (ms: number) => Promise<void>;
	fs: ReleaseAssetsFs;
	/** Prints a line to stdout. */
	print: (line: string) => void;
	/** Prints a line to stderr. */
	print_error: (line: string) => void;
}

/**
 * Collect `v<version>`'s release assets into `--out` (module doc).
 * @returns the exit code: 0 once every asset, `SHA512SUMS` and the NOTICE are written, 1 for a
 *   usage error, a tag that is no version, or any failure collecting them (printed as
 *   `FAIL: <reason>`, the assets written so far left in `--out`)
 * @throws `TypeError` - for a flag `parseArgs` refuses
 */
export async function run_release_assets(
	deps: ReleaseAssetsDeps,
	argv: Array<string>
): Promise<number> {
	const { values: args, positionals } = parseArgs({
		args: argv,
		allowPositionals: true,
		options: {
			out: { type: 'string' }
		}
	});
	const [tag] = positionals;
	if (!tag || !args.out) {
		deps.print_error('usage: release_assets.ts v<version> --out <dir>');
		return 1;
	}
	const version = version_from_tag(tag);
	if (version === null) {
		deps.print_error(`FAIL: "${tag}" is not a v<major>.<minor>.<patch> version`);
		return 1;
	}
	try {
		await collect(deps, version, resolve(deps.cwd, args.out));
		return 0;
	} catch (error) {
		deps.print_error(`FAIL: ${error instanceof Error ? error.message : String(error)}`);
		return 1;
	}
}

/** A captured run read as the release scripts read one: success is exit 0, stdout keeps its
 * leading whitespace, stderr is trimmed both ends. */
const capture_out = (run: CapturedRun): { ok: boolean; out: string; err: string } => ({
	ok: run.code === 0,
	out: run.stdout.trimEnd(),
	err: run.stderr.trim()
});

const collect = async (deps: ReleaseAssetsDeps, version: string, out_dir: string) => {
	const { fs } = deps;

	/** A registry read: every failure is retried on the bounded window above (loudly, per
	 * attempt), since the registry is the only side that can be transiently behind. */
	const capture_npm = async (npm_args: Array<string>): Promise<string> => {
		for (let attempt = 1; ; attempt++) {
			const result = capture_out(await deps.npm(npm_args));
			if (result.ok) return result.out;
			if (attempt >= REGISTRY_ATTEMPTS) {
				throw new Error(
					`npm ${npm_args.join(' ')} failed after ${attempt} attempts:\n${result.err}`
				);
			}
			deps.print_error(
				`  npm ${npm_args.join(' ')} failed (attempt ${attempt}/${REGISTRY_ATTEMPTS}) — retrying in ${REGISTRY_RETRY_MS / 1000}s`
			);
			await deps.sleep(REGISTRY_RETRY_MS);
		}
	};

	// The published loader's platform set — the registry's own statement of which
	// packages exist at this version, and a check that the loader itself is live.
	// An exact-version query whose field is absent prints NOTHING (exit 0), not
	// `{}`, so the empty case is read before the parse.
	const raw = await capture_npm(['view', `${LOADER}@${version}`, 'optionalDependencies', '--json']);
	const optional = (raw === '' ? {} : JSON.parse(raw)) as Record<string, string>;
	const platform_pkgs = Object.keys(optional);
	if (platform_pkgs.length === 0) {
		throw new Error(
			`${LOADER}@${version} names no optionalDependencies — is the published loader the staged one?`
		);
	}

	await fs.make_dir(out_dir);
	const pack_dir = await fs.make_temp_dir(PACK_DIR_PREFIX);
	const asset_names: Array<string> = [];
	try {
		for (const pkg of platform_pkgs) {
			const triple = PLATFORM_PACKAGE.exec(pkg)?.[1];
			if (triple === undefined) {
				throw new Error(
					`${LOADER}@${version} names ${pkg}, which is not a ${LOADER}-<triple> platform package`
				);
			}
			const pinned = optional[pkg];
			if (pinned !== version) {
				throw new Error(
					`${LOADER}@${version} pins ${pkg}@${pinned} — versions must move in lockstep`
				);
			}
			const binary = cli_binary_name(triple);
			const asset = release_asset_name(triple);

			// `--min-release-age=0`: npm reads the checkout's `.npmrc`, whose release-age window some
			// npm versions apply to `npm pack` past its `@fuzdev/*` exclusion, and the version this
			// packs was published moments ago, so it would read as no matching version; a flag
			// outranks every config file and the environment
			const packed = JSON.parse(
				await capture_npm([
					'pack',
					`${pkg}@${version}`,
					'--min-release-age=0',
					'--pack-destination',
					pack_dir,
					'--json'
				])
			) as Array<{ filename?: string }>;
			const filename = packed[0]?.filename;
			if (!filename) throw new Error(`npm pack ${pkg}@${version} reported no tarball`);
			if (filename === '.' || filename === '..' || /[/\\]/.test(filename)) {
				throw new Error(`npm pack ${pkg}@${version} reported ${filename}, which is no file name`);
			}
			const tar_args = tar_extract_args(`${pack_dir}/${filename}`, pack_dir, `package/${binary}`);
			const extracted = capture_out(await deps.tar(tar_args));
			if (!extracted.ok) throw new Error(`tar ${tar_args.join(' ')} failed:\n${extracted.err}`);
			const extracted_path = `${pack_dir}/package/${binary}`;
			if (!(await fs.is_regular_file(extracted_path))) {
				throw new Error(`package/${binary} in ${pkg}@${version} is not a regular file`);
			}
			// A copy, not a rename: the temp dir and `--out` can sit on different
			// filesystems (a tmpfs /tmp is common), where rename(2) is EXDEV.
			await fs.copy_file(extracted_path, join(out_dir, asset));
			asset_names.push(asset);
			const size = await fs.size(join(out_dir, asset));
			deps.print(`  ${asset}  (${pkg}@${version}, ${(size / 1024 / 1024).toFixed(2)} MB)`);
		}
	} finally {
		await fs.remove_dir(pack_dir);
	}

	// `sha512sum -c SHA512SUMS` format: hex, two spaces, name
	const sums: Array<string> = [];
	for (const asset of asset_names) {
		const bytes = await fs.read_bytes(join(out_dir, asset));
		const digest = await crypto.subtle.digest('SHA-512', bytes);
		const hex = Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, '0')).join('');
		sums.push(`${hex}  ${asset}`);
	}
	await fs.write_text(join(out_dir, 'SHA512SUMS'), sums.join('\n') + '\n');
	deps.print(`  SHA512SUMS  (${asset_names.length} assets)`);

	// The binaries' third-party notice, as the documentation BSD 3-Clause asks a binary
	// distribution to carry: the same file every platform package ships beside the binary.
	await fs.copy_file(
		join(deps.checkout, THIRD_PARTY_NOTICE.source),
		join(out_dir, THIRD_PARTY_NOTICE.published)
	);
	deps.print(`  ${THIRD_PARTY_NOTICE.published}  (${THIRD_PARTY_NOTICE.source})`);
};
