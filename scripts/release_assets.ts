/**
 * Collect the GitHub Release assets for one version: the native `tsv` CLI
 * binary of every `@fuzdev/tsv-<triple>` platform package, plus a `SHA256SUMS`
 * file (`sha256sum -c` format). The release workflow's `release` job runs this
 * after the npm publish and uploads the directory.
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
 * Usage: deno run --allow-read --allow-write --allow-run=npm,tar \
 *          scripts/release_assets.ts v<version> --out <dir>
 *        deno task release:assets v0.3.0 --out /tmp/tsv_assets
 *
 * Re-runnable locally against any published version, which is also how a
 * release's `SHA256SUMS` can be re-derived from npm and compared.
 */

import { parseArgs } from 'node:util';

import { version_from_tag } from './changelog.ts';
import { cli_binary_name } from './napi_host.ts';

const { values: args, positionals } = parseArgs({
	allowPositionals: true,
	options: {
		out: { type: 'string' }
	}
});

const [tag] = positionals;
if (!tag || !args.out) {
	console.error('usage: release_assets.ts v<version> --out <dir>');
	Deno.exit(1);
}
const version = version_from_tag(tag);
if (version === null) {
	console.error(`FAIL: "${tag}" is not a v<major>.<minor>.<patch> version`);
	Deno.exit(1);
}
const out_dir = args.out;

const LOADER = '@fuzdev/tsv';

/** How long a registry read is given to see a just-published version:
 * `REGISTRY_ATTEMPTS` tries, `REGISTRY_RETRY_MS` apart (about ten minutes of
 * waiting in total). A first-ever package's packument has lagged the read path
 * past two and a half minutes (v0.3.0's bootstrap release), so the window is
 * sized for that case; the workflow's job timeout must stay above it. */
const REGISTRY_ATTEMPTS = 30;
const REGISTRY_RETRY_MS = 20_000;

const dec = new TextDecoder();

interface Captured {
	success: boolean;
	stdout: string;
	stderr: string;
}

const run_captured = async (cmd: string, cmd_args: Array<string>): Promise<Captured> => {
	const result = await new Deno.Command(cmd, {
		args: cmd_args,
		stdout: 'piped',
		stderr: 'piped'
	}).output();
	return {
		success: result.success,
		stdout: dec.decode(result.stdout).trim(),
		stderr: dec.decode(result.stderr)
	};
};

/** Run a command once; its trimmed stdout, or a thrown error naming it. */
const capture = async (cmd: string, cmd_args: Array<string>): Promise<string> => {
	const result = await run_captured(cmd, cmd_args);
	if (!result.success) {
		throw new Error(`${cmd} ${cmd_args.join(' ')} failed:\n${result.stderr}`);
	}
	return result.stdout;
};

/** `capture` for the registry: every failure is retried on the bounded window
 * above (loudly, per attempt), since the registry is the only side that can be
 * transiently behind. */
const capture_npm = async (npm_args: Array<string>): Promise<string> => {
	for (let attempt = 1; ; attempt++) {
		const result = await run_captured('npm', npm_args);
		if (result.success) return result.stdout;
		if (attempt >= REGISTRY_ATTEMPTS) {
			throw new Error(
				`npm ${npm_args.join(' ')} failed after ${attempt} attempts:\n${result.stderr}`
			);
		}
		console.warn(
			`  npm ${npm_args.join(' ')} failed (attempt ${attempt}/${REGISTRY_ATTEMPTS}) — retrying in ${REGISTRY_RETRY_MS / 1000}s`
		);
		await new Promise((resolve) => setTimeout(resolve, REGISTRY_RETRY_MS));
	}
};

const collect = async (): Promise<void> => {
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

	Deno.mkdirSync(out_dir, { recursive: true });
	const pack_dir = Deno.makeTempDirSync({ prefix: 'tsv_release_assets_' });
	const asset_names: Array<string> = [];
	try {
		for (const pkg of platform_pkgs) {
			if (!pkg.startsWith(`${LOADER}-`)) {
				throw new Error(
					`${LOADER}@${version} names ${pkg}, which is not a ${LOADER}-<triple> platform package`
				);
			}
			const triple = pkg.slice(`${LOADER}-`.length);
			const pinned = optional[pkg];
			if (pinned !== version) {
				throw new Error(
					`${LOADER}@${version} pins ${pkg}@${pinned} — versions must move in lockstep`
				);
			}
			const binary = cli_binary_name(triple);
			// the asset is the binary's own name with the triple spliced in, so the
			// two keep one extension rule: `tsv` → `tsv-linux-x64-gnu`, `tsv.exe` →
			// `tsv-win32-x64.exe`
			const asset = binary.replace(/^tsv/, `tsv-${triple}`);

			const packed = JSON.parse(
				await capture_npm(['pack', `${pkg}@${version}`, '--pack-destination', pack_dir, '--json'])
			) as Array<{ filename?: string }>;
			const filename = packed[0]?.filename;
			if (!filename) throw new Error(`npm pack ${pkg}@${version} reported no tarball`);
			await capture('tar', [
				'-xzf',
				`${pack_dir}/${filename}`,
				'-C',
				pack_dir,
				`package/${binary}`
			]);
			// A copy, not a rename: the temp dir and `--out` can sit on different
			// filesystems (a tmpfs /tmp is common), where rename(2) is EXDEV.
			Deno.copyFileSync(`${pack_dir}/package/${binary}`, `${out_dir}/${asset}`);
			asset_names.push(asset);
			const size = Deno.statSync(`${out_dir}/${asset}`).size;
			console.log(`  ${asset}  (${pkg}@${version}, ${(size / 1024 / 1024).toFixed(2)} MB)`);
		}
	} finally {
		Deno.removeSync(pack_dir, { recursive: true });
	}

	// `sha256sum -c SHA256SUMS` format: hex, two spaces, name.
	const sums: Array<string> = [];
	for (const asset of asset_names) {
		const bytes = Deno.readFileSync(`${out_dir}/${asset}`);
		const digest = await crypto.subtle.digest('SHA-256', bytes);
		const hex = Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, '0')).join('');
		sums.push(`${hex}  ${asset}`);
	}
	Deno.writeTextFileSync(`${out_dir}/SHA256SUMS`, sums.join('\n') + '\n');
	console.log(`  SHA256SUMS  (${asset_names.length} assets)`);
};

try {
	await collect();
} catch (error) {
	console.error(`FAIL: ${error instanceof Error ? error.message : String(error)}`);
	Deno.exit(1);
}
