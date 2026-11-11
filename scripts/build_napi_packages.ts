/**
 * Stage the publishable N-API npm packages into `crates/tsv_napi/pkg/`:
 *
 * - `pkg/napi/` — the `@fuzdev/tsv` loader, ESM like the wasm packages (index.js + index.d.ts +
 *   platform.js (triple detection) + the shared facade with its parse, strip and imports
 *   halves — `api.js` / `api_parse.js` / `api_strip.js` / `api_imports.js`, the `locations.js`
 *   helper, the declarations (`facade_format.d.ts`, `facade_parse.d.ts`,
 *   `facade_strip.d.ts`, `facade_imports.d.ts`, `facade_error.d.ts`, `locations.d.ts`,
 *   `tsv_ast.d.ts`), the table
 *   `scripts/npm_facade.ts` names — + the shared `cli.js` + the `bin.js` dispatcher wired
 *   as the `tsv` bin + README + LICENSE + the third-party NOTICE + generated package.json
 *   with the exact-pinned platform `optionalDependencies`).
 * - `pkg/<triple>/` — ONE platform package, `@fuzdev/tsv-<triple>`: the
 *   built cdylib copied to `tsv_napi.node` (a byte-identical rename), the
 *   real `tsv_cli` binary copied to `tsv`/`tsv.exe` (what `bin.js` execs —
 *   `npx tsv` on this set runs the native CLI), the LICENSE and the
 *   third-party NOTICE both binaries need, plus a generated
 *   package.json whose `os`/`cpu`/`libc` fields drive install-time
 *   selection.
 *
 * One platform per invocation by design: a machine can only have built its own
 * triple, and the release workflow runs this once per matrix target
 * (`--triple` + `--artifact`/`--cli-artifact` name the binaries to stage — the PGO
 * pipeline's, under `.pgo/`; defaults are the host triple, `target/napi/`, and
 * `target/release/`).
 * `--loader-only` skips the platform half entirely — the release workflow's
 * publish job stages the loader beside downloaded platform artifacts, on a
 * machine that built nothing. The cargo builds themselves are the
 * `build:napi` task + `cargo build -p tsv_cli --release`, chained by
 * `deno task build:napi:packages` — this script only stages files.
 *
 * Its entry is `scripts/roots/build_napi_packages.ts`, whose module doc says why this module
 * and everything it imports stay on `node:*` and relative modules, never a package. Every
 * path it stages beneath is the checkout's, never the working directory's; an `--artifact` or
 * `--cli-artifact` is read as given.
 *
 * Usage: node scripts/roots/build_napi_packages.ts [--triple <t>] [--artifact <path>] \
 *          [--cli-artifact <path>] [--loader-only]
 *
 * Exits 0 once staged, 1 when a built binary can't be read; a bad flag throws, and so does a
 * triple outside `SUPPORTED_TRIPLES` unless `--loader-only`, before anything is written: the
 * triple names the platform package's directory beneath `pkg/`, so only a listed one is staged.
 *
 * @module
 */

import { join } from 'node:path';
import { parseArgs } from 'node:util';

import { cli_binary_name } from './napi_host.ts';
import { ALL_FAMILIES, LOCATIONS_EXPORT, facade_files } from './npm_facade.ts';
import { NPM_SHARED_METADATA, THIRD_PARTY_NOTICE } from './npm_metadata.ts';
import { cargo_workspace_version } from './release/version.ts';
import { format_size } from './size.ts';
import { stage_file, type StageFileDeps } from './stage_file.ts';

/** The platform packages the loader pins — keep in sync with `npm/index.js`'s
 * `SUPPORTED` (`scripts/test_napi_npm.ts` gates the agreement, via the
 * generated optionalDependencies). */
export const SUPPORTED_TRIPLES = [
	'linux-x64-gnu',
	'linux-arm64-gnu',
	'linux-x64-musl',
	'darwin-arm64',
	'darwin-x64',
	'win32-x64'
];

/** package.json `os`/`cpu`/`libc` selection fields for a triple. */
export const platform_fields = (
	triple: string
): { os: string[]; cpu: string[]; libc?: string[] } => {
	const [os, cpu, libc] = triple.split('-');
	if (!os || !cpu) throw new Error(`malformed triple '${triple}'`);
	return {
		os: [os],
		cpu: [cpu],
		...(libc ? { libc: [libc === 'gnu' ? 'glibc' : libc] } : {})
	};
};

/** The cdylib's filename as cargo writes it on `platform` (Node's `process.platform`). */
export const native_library_filename = (platform: string): string => {
	const ext = platform === 'darwin' ? 'dylib' : platform === 'win32' ? 'dll' : 'so';
	const prefix = platform === 'win32' ? '' : 'lib';
	return `${prefix}tsv_napi.${ext}`;
};

/** The files the staging reaches, by path (the checkout's, or a binary's as given). */
export interface NapiStagingFs extends StageFileDeps {
	read_text: (path: string) => Promise<string>;
	write_text: (path: string, text: string) => Promise<void>;
	/** Creates the directory and any missing parents. */
	make_dir: (path: string) => Promise<void>;
	set_mode: (path: string, mode: number) => Promise<void>;
	/** The file's size in bytes. */
	size: (path: string) => Promise<number>;
}

/** What `run_build_napi_packages` reaches. */
export interface BuildNapiPackagesDeps {
	/** The checkout's path: the sources, `Cargo.toml`, the default binaries, and `pkg/`. */
	checkout: string;
	/** The host as Node names it (`process.platform`): the default binaries' names, and
	 * whether the CLI binary's mode is set. */
	platform: string;
	/** The host's platform triple, staged when `--triple` isn't given. */
	host_triple: string;
	fs: NapiStagingFs;
	print: (line: string) => void;
	print_error: (line: string) => void;
}

/**
 * Stage the loader package and, unless `--loader-only`, the one platform package (module doc).
 * @returns the exit code: 0 once staged, 1 when a built binary can't be read
 * @throws `TypeError` - for a flag `parseArgs` refuses
 * @throws `Error` - for a triple outside `SUPPORTED_TRIPLES` (unless `--loader-only`), before
 *   anything is read, or for a `Cargo.toml` with no workspace version
 */
export async function run_build_napi_packages(
	deps: BuildNapiPackagesDeps,
	argv: Array<string>
): Promise<number> {
	const { fs } = deps;
	const { values: args } = parseArgs({
		args: argv,
		options: {
			triple: { type: 'string' },
			artifact: { type: 'string' },
			'cli-artifact': { type: 'string' },
			'loader-only': { type: 'boolean' }
		}
	});
	const at = (path: string): string => join(deps.checkout, path);

	const triple = args.triple ?? deps.host_triple;
	// the triple is a directory name beneath `pkg/`, so membership fences every write to it
	if (!args['loader-only'] && !SUPPORTED_TRIPLES.includes(triple)) {
		throw new Error(`unsupported triple '${triple}' (supported: ${SUPPORTED_TRIPLES.join(', ')})`);
	}
	// each binary as a flag names it (read as given) or as the checkout's default build
	const artifact = args.artifact ?? `target/napi/${native_library_filename(deps.platform)}`;
	const artifact_path = args.artifact ?? at(artifact);
	// The staged NAME follows the triple; the default SOURCE path follows the host
	// (every matrix leg builds natively or in a same-arch container, so the two agree).
	const cli_binary = cli_binary_name(triple);
	const cli_artifact =
		args['cli-artifact'] ?? `target/release/${deps.platform === 'win32' ? 'tsv.exe' : 'tsv'}`;
	const cli_artifact_path = args['cli-artifact'] ?? at(cli_artifact);

	const cargo_toml = await fs.read_text(at('Cargo.toml'));
	// the one `[workspace.package]` version reader, shared with publish's bump and the bench report
	const version = cargo_workspace_version(cargo_toml);
	if (!version) throw new Error('workspace version not found in Cargo.toml');

	const write_pkg = (dir: string, pkg: Record<string, unknown>): Promise<void> =>
		fs.write_text(at(`${dir}/package.json`), JSON.stringify(pkg, null, '\t') + '\n');

	// --- the loader package -----------------------------------------------------

	const loader_dir = 'crates/tsv_napi/pkg/napi';
	await fs.make_dir(at(loader_dir));
	for (const [from, to] of [
		['crates/tsv_napi/npm/index.js', 'index.js'],
		['crates/tsv_napi/npm/index.d.ts', 'index.d.ts'],
		['crates/tsv_napi/npm/platform.js', 'platform.js'],
		// the `tsv` bin — a dispatcher that execs the platform package's native
		// CLI binary, falling back to cli.js (see crates/tsv_napi/npm/bin.js)
		['crates/tsv_napi/npm/bin.js', 'bin.js'],
		['crates/tsv_napi/npm/README.md', 'README.md'],
		// the shared facade every tsv package exports through — one options reader and
		// one set of error texts with the wasm packages; `index.js` imports every half
		...facade_files(ALL_FAMILIES).map(({ source, published }) => [source, published]),
		// the JS CLI — imports its engine from `./index.js`, so the same source
		// binds to the native loader here and to the wasm engine in
		// @fuzdev/tsv-wasm (where it IS the bin); here it is bin.js's fallback
		['crates/tsv_wasm/npm/cli.js', 'cli.js'],
		['LICENSE', 'LICENSE'],
		// the WHATWG notice for the entity table the platform binaries compile in (see
		// THIRD_PARTY_NOTICE); the loader ships it too, as the package a consumer installs
		[THIRD_PARTY_NOTICE.source, THIRD_PARTY_NOTICE.published]
	]) {
		await stage_file(fs, at(from), at(`${loader_dir}/${to}`));
	}

	await write_pkg(loader_dir, {
		name: '@fuzdev/tsv',
		version,
		description:
			'native formatter and parser for Svelte, TypeScript, and CSS, a TypeScript type stripper, ' +
			'and an import-records parser (N-API)',
		// ESM, matching the wasm packages — one module system across tsv's npm
		// surface, so shared sources load unchanged in either. The platform `.node`
		// is still `require`d, via `createRequire` (see npm/index.js).
		type: 'module',
		main: 'index.js',
		types: 'index.d.ts',
		exports: {
			'./package.json': './package.json',
			'.': {
				types: './index.d.ts',
				default: './index.js'
			},
			// the reconstruction helper alone — pure JS that imports nothing, so it loads no
			// addon (the wasm packages that parse export the same subpath)
			'./locations': LOCATIONS_EXPORT
		},
		// Bare relative path: npm normalizes `./bin.js` → `bin.js` at publish and words
		// that normalization as the bin being "invalid and removed" (a false alarm) —
		// write the normalized form so the publish log stays quiet.
		bin: { tsv: 'bin.js' },
		files: [
			'index.js',
			'index.d.ts',
			'platform.js',
			'bin.js',
			...facade_files(ALL_FAMILIES).map((file) => file.published),
			'cli.js',
			'README.md',
			'LICENSE',
			THIRD_PARTY_NOTICE.published
		],
		keywords: [
			'typescript',
			'svelte',
			'css',
			'formatter',
			'strip',
			'imports',
			'prettier',
			'parser',
			'ast',
			'acorn',
			'cli',
			'napi',
			'native'
		],
		...NPM_SHARED_METADATA,
		// The loader's top-level platform require IS the side effect.
		sideEffects: ['./index.js'],
		optionalDependencies: Object.fromEntries(
			SUPPORTED_TRIPLES.map((t) => [`@fuzdev/tsv-${t}`, version])
		)
	});
	deps.print(`Staged ${loader_dir}: @fuzdev/tsv ${version}`);

	if (args['loader-only']) return 0;

	// --- the platform package ---------------------------------------------------

	const platform_dir = `crates/tsv_napi/pkg/${triple}`;
	await fs.make_dir(at(platform_dir));
	/** Stage a built binary, or say what builds it; `from` is the path the message names. */
	const copy_built = async (
		what: string,
		from: string,
		from_path: string,
		to: string,
		build_command: string
	): Promise<boolean> => {
		try {
			await stage_file(fs, from_path, at(to));
			return true;
		} catch (e) {
			deps.print_error(`FAIL: cannot read ${what} at ${from} — run '${build_command}' first`);
			deps.print_error(String(e));
			return false;
		}
	};
	if (
		!(await copy_built(
			'the built cdylib',
			artifact,
			artifact_path,
			`${platform_dir}/tsv_napi.node`,
			'deno task build:napi'
		)) ||
		!(await copy_built(
			'the built CLI binary',
			cli_artifact,
			cli_artifact_path,
			`${platform_dir}/${cli_binary}`,
			'cargo build -p tsv_cli --release'
		))
	) {
		return 1;
	}
	if (deps.platform !== 'win32') {
		// npm packs the on-disk mode into the tarball, and npm only chmods `bin`
		// entries (the loader's bin.js) — this binary must carry its own x-bit
		await fs.set_mode(at(`${platform_dir}/${cli_binary}`), 0o755);
	}
	await stage_file(fs, at('LICENSE'), at(`${platform_dir}/LICENSE`));
	await stage_file(
		fs,
		at(THIRD_PARTY_NOTICE.source),
		at(`${platform_dir}/${THIRD_PARTY_NOTICE.published}`)
	);
	await fs.write_text(
		at(`${platform_dir}/README.md`),
		`# @fuzdev/tsv-${triple}\n\n` +
			`> prebuilt tsv N-API binding for ${triple}\n\n` +
			`Platform binaries for [\`@fuzdev/tsv\`](https://www.npmjs.com/package/@fuzdev/tsv) — the N-API addon plus the native \`tsv\` CLI its bin runs. Install that package instead; it selects this one automatically.\n`
	);
	await write_pkg(platform_dir, {
		name: `@fuzdev/tsv-${triple}`,
		version,
		description: `prebuilt tsv N-API binding for ${triple}`,
		main: 'tsv_napi.node',
		files: ['tsv_napi.node', cli_binary, 'README.md', 'LICENSE', THIRD_PARTY_NOTICE.published],
		...platform_fields(triple),
		...NPM_SHARED_METADATA
	});
	const size = await fs.size(at(`${platform_dir}/tsv_napi.node`));
	const cli_size = await fs.size(at(`${platform_dir}/${cli_binary}`));
	deps.print(
		`Staged ${platform_dir}: @fuzdev/tsv-${triple} ${version} (tsv_napi.node ${format_size(size)}, ${cli_binary} ${format_size(cli_size)})`
	);
	return 0;
}
