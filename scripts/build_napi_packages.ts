/**
 * Stage the publishable N-API npm packages into `crates/tsv_napi/pkg/`:
 *
 * - `pkg/napi/` — the `@fuzdev/tsv` loader, ESM like the wasm packages (index.js + index.d.ts +
 *   tsv_ast.d.ts + the shared `locations.js`/`.d.ts` helper + the shared
 *   `cli.js` + the `bin.js` dispatcher wired as the `tsv` bin + README +
 *   LICENSE + generated package.json with the exact-pinned platform
 *   `optionalDependencies`).
 * - `pkg/<triple>/` — ONE platform package, `@fuzdev/tsv-<triple>`: the
 *   built cdylib copied to `tsv_napi.node` (a byte-identical rename), the
 *   real `tsv_cli` binary copied to `tsv`/`tsv.exe` (what `bin.js` execs —
 *   `npx tsv` on this set runs the native CLI), plus a generated
 *   package.json whose `os`/`cpu`/`libc` fields drive install-time
 *   selection.
 *
 * One platform per invocation by design: a machine can only have built its own
 * triple, and the release workflow runs this once per matrix target
 * (`--triple` + `--artifact`/`--cli-artifact` name cross-built binaries;
 * defaults are the host triple, `target/napi/`, and `target/release/`).
 * `--loader-only` skips the platform half entirely — the release workflow's
 * publish job stages the loader beside downloaded platform artifacts, on a
 * machine that built nothing. The cargo builds themselves are the
 * `build:napi` task + `cargo build -p tsv_cli --release`, chained by
 * `deno task build:napi:packages` — this script only stages files.
 *
 * Usage: deno run --allow-read --allow-write=crates/tsv_napi/pkg \
 *          scripts/build_napi_packages.ts [--triple <t>] [--artifact <path>] \
 *          [--cli-artifact <path>] [--loader-only]
 */

import { parseArgs } from 'node:util';

import { NPM_SHARED_METADATA } from './npm_metadata.ts';

const { values: args } = parseArgs({
	options: {
		triple: { type: 'string' },
		artifact: { type: 'string' },
		'cli-artifact': { type: 'string' },
		'loader-only': { type: 'boolean' }
	}
});

/** The platform packages the loader pins — keep in sync with `npm/index.js`'s
 * `SUPPORTED` (`scripts/test_napi_npm.ts` gates the agreement, via the
 * generated optionalDependencies). Not exported: importing this module runs
 * the staging. */
const SUPPORTED_TRIPLES = [
	'linux-x64-gnu',
	'linux-arm64-gnu',
	'linux-x64-musl',
	'darwin-arm64',
	'win32-x64'
];

/** Host triple in node-platform terms, from Deno's own build target. */
const host_triple = (): string => {
	const { os, arch, target } = Deno.build;
	const cpu = arch === 'x86_64' ? 'x64' : arch === 'aarch64' ? 'arm64' : arch;
	if (os === 'linux') return `linux-${cpu}-${target.includes('musl') ? 'musl' : 'gnu'}`;
	if (os === 'darwin') return `darwin-${cpu}`;
	if (os === 'windows') return `win32-${cpu}`;
	return `${os}-${cpu}`;
};

/** package.json `os`/`cpu`/`libc` selection fields for a triple. */
const platform_fields = (triple: string): { os: string[]; cpu: string[]; libc?: string[] } => {
	const [os, cpu, libc] = triple.split('-');
	if (!os || !cpu) throw new Error(`malformed triple '${triple}'`);
	return {
		os: [os],
		cpu: [cpu],
		...(libc ? { libc: [libc === 'gnu' ? 'glibc' : libc] } : {})
	};
};

const native_library_filename = (): string => {
	const { os } = Deno.build;
	const ext = os === 'darwin' ? 'dylib' : os === 'windows' ? 'dll' : 'so';
	const prefix = os === 'windows' ? '' : 'lib';
	return `${prefix}tsv_napi.${ext}`;
};

const triple = args.triple ?? host_triple();
const artifact = args.artifact ?? `target/napi/${native_library_filename()}`;
// The staged NAME follows the target triple; the default SOURCE path follows
// the host (every matrix leg builds natively or in a same-arch container, so
// the two agree — a genuine cross-build names its binary via --cli-artifact).
const cli_binary_name = triple.startsWith('win32-') ? 'tsv.exe' : 'tsv';
const cli_artifact =
	args['cli-artifact'] ?? `target/release/${Deno.build.os === 'windows' ? 'tsv.exe' : 'tsv'}`;

const cargo_toml = Deno.readTextFileSync('Cargo.toml');
const version = /\[workspace\.package\][^[]*?version = "([^"]+)"/.exec(cargo_toml)?.[1];
if (!version) throw new Error('workspace version not found in Cargo.toml');

const write_pkg = (dir: string, pkg: Record<string, unknown>): void => {
	Deno.writeTextFileSync(`${dir}/package.json`, JSON.stringify(pkg, null, '\t') + '\n');
};

// --- the loader package -----------------------------------------------------

const loader_dir = 'crates/tsv_napi/pkg/napi';
Deno.mkdirSync(loader_dir, { recursive: true });
for (const [from, to] of [
	['crates/tsv_napi/npm/index.js', 'index.js'],
	['crates/tsv_napi/npm/index.d.ts', 'index.d.ts'],
	['crates/tsv_napi/npm/platform.js', 'platform.js'],
	// the `tsv` bin — a dispatcher that execs the platform package's native
	// CLI binary, falling back to cli.js (see crates/tsv_napi/npm/bin.js)
	['crates/tsv_napi/npm/bin.js', 'bin.js'],
	['crates/tsv_napi/npm/README.md', 'README.md'],
	['crates/tsv_wasm/types/tsv_ast.d.ts', 'tsv_ast.d.ts'],
	['crates/tsv_wasm/npm/locations.js', 'locations.js'],
	['crates/tsv_wasm/npm/locations.d.ts', 'locations.d.ts'],
	// the JS CLI — imports its engine from `./index.js`, so the same source
	// binds to the native loader here and to the wasm engine in
	// @fuzdev/tsv_wasm (where it IS the bin); here it is bin.js's fallback
	['crates/tsv_wasm/npm/cli.js', 'cli.js'],
	['LICENSE', 'LICENSE']
]) {
	Deno.copyFileSync(from, `${loader_dir}/${to}`);
}

// The `no-locations` reconstruction helpers are pure JS over the span-only
// wire — no wasm, no addon — so the wasm packages' copy ships here verbatim,
// which is what the ESM loader bought. The re-export is APPENDED to the staged
// entry rather than written into `npm/index.js`: the source tree has no
// sibling `locations.js`, and an import that resolves only after staging is a
// wart an editor would flag. Names are extracted from the helper rather than
// listed here, so a fourth entry point can't reach one package and miss the
// other (`scripts/patch_npm_package.ts` holds the wasm side's copy).
const locations_source = Deno.readTextFileSync(`${loader_dir}/locations.js`);
const locations_exports = [...locations_source.matchAll(/^export function (\w+)/gm)].map(
	(m) => m[1]
);
if (!locations_exports.length) {
	console.error('FAIL: no exports found in locations.js — did the helper change shape?');
	Deno.exit(1);
}
Deno.writeTextFileSync(
	`${loader_dir}/index.js`,
	`\nexport { ${locations_exports.join(', ')} } from './locations.js';\n`,
	{ append: true }
);
// `export *`, not `export type *` — the helper's functions AND its types flow
// through, matching the wasm packages' index.d.ts.
Deno.writeTextFileSync(`${loader_dir}/index.d.ts`, `\nexport * from './locations';\n`, {
	append: true
});
write_pkg(loader_dir, {
	name: '@fuzdev/tsv',
	version,
	description: 'native formatter and parser for Svelte, TypeScript, and CSS (N-API)',
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
		}
	},
	bin: { tsv: './bin.js' },
	files: [
		'index.js',
		'index.d.ts',
		'platform.js',
		'bin.js',
		'locations.js',
		'locations.d.ts',
		'tsv_ast.d.ts',
		'cli.js',
		'README.md',
		'LICENSE'
	],
	keywords: [
		'typescript',
		'svelte',
		'css',
		'formatter',
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
console.log(`Staged ${loader_dir}: @fuzdev/tsv ${version}`);

if (args['loader-only']) {
	Deno.exit(0);
}

// --- the platform package ---------------------------------------------------

const platform_dir = `crates/tsv_napi/pkg/${triple}`;
Deno.mkdirSync(platform_dir, { recursive: true });
const copy_built = (what: string, from: string, to: string, build_command: string): void => {
	try {
		Deno.copyFileSync(from, to);
	} catch (e) {
		console.error(`FAIL: cannot read ${what} at ${from} — run '${build_command}' first`);
		console.error(String(e));
		Deno.exit(1);
	}
};
copy_built('the built cdylib', artifact, `${platform_dir}/tsv_napi.node`, 'deno task build:napi');
copy_built(
	'the built CLI binary',
	cli_artifact,
	`${platform_dir}/${cli_binary_name}`,
	'cargo build -p tsv_cli --release'
);
if (Deno.build.os !== 'windows') {
	// npm packs the on-disk mode into the tarball, and npm only chmods `bin`
	// entries (the loader's bin.js) — this binary must carry its own x-bit
	Deno.chmodSync(`${platform_dir}/${cli_binary_name}`, 0o755);
}
Deno.copyFileSync('LICENSE', `${platform_dir}/LICENSE`);
Deno.writeTextFileSync(
	`${platform_dir}/README.md`,
	`# @fuzdev/tsv-${triple}\n\n` +
		`> prebuilt tsv N-API binding for ${triple}\n\n` +
		`Platform binaries for [\`@fuzdev/tsv\`](https://www.npmjs.com/package/@fuzdev/tsv) — the N-API addon plus the native \`tsv\` CLI its bin runs. Install that package instead; it selects this one automatically.\n`
);
write_pkg(platform_dir, {
	name: `@fuzdev/tsv-${triple}`,
	version,
	description: `prebuilt tsv N-API binding for ${triple}`,
	main: 'tsv_napi.node',
	files: ['tsv_napi.node', cli_binary_name, 'README.md', 'LICENSE'],
	...platform_fields(triple),
	...NPM_SHARED_METADATA
});
const size = Deno.statSync(`${platform_dir}/tsv_napi.node`).size;
const cli_size = Deno.statSync(`${platform_dir}/${cli_binary_name}`).size;
console.log(
	`Staged ${platform_dir}: @fuzdev/tsv-${triple} ${version} (tsv_napi.node ${(size / 1024 / 1024).toFixed(1)} MB, ${cli_binary_name} ${(cli_size / 1024 / 1024).toFixed(1)} MB)`
);
