/**
 * Stage the publishable N-API npm packages into `crates/tsv_napi/pkg/`:
 *
 * - `pkg/napi/` — the `@fuzdev/tsv` loader (index.js + index.d.ts +
 *   tsv_ast.d.ts + README + LICENSE + generated package.json with the
 *   exact-pinned platform `optionalDependencies`).
 * - `pkg/<triple>/` — ONE platform package, `@fuzdev/tsv-<triple>`: the
 *   built cdylib copied to `tsv_napi.node` (a byte-identical rename) plus a
 *   generated package.json whose `os`/`cpu`/`libc` fields drive install-time
 *   selection.
 *
 * One platform per invocation by design: a machine can only have built its own
 * triple, and the release workflow runs this once per matrix target
 * (`--triple` + `--artifact` name the cross-built binary; defaults are the
 * host triple and `target/napi/`). `--loader-only` skips the platform half
 * entirely — the release workflow's publish job stages the loader beside
 * downloaded platform artifacts, on a machine that built nothing. The cargo
 * build itself is the `build:napi` task, chained by
 * `deno task build:napi:packages` — this script only stages files.
 *
 * Usage: deno run --allow-read --allow-write=crates/tsv_napi/pkg \
 *          scripts/build_napi_packages.ts [--triple <t>] [--artifact <path>] [--loader-only]
 */

import { parseArgs } from 'node:util';

const { values: args } = parseArgs({
	options: {
		triple: { type: 'string' },
		artifact: { type: 'string' },
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

const cargo_toml = Deno.readTextFileSync('Cargo.toml');
const version = /\[workspace\.package\][^[]*?version = "([^"]+)"/.exec(cargo_toml)?.[1];
if (!version) throw new Error('workspace version not found in Cargo.toml');

// Shared npm metadata — mirrors scripts/patch_npm_package.ts so the napi and
// wasm packages present identically on the registry.
const shared_metadata = {
	license: 'MIT',
	homepage: 'https://github.com/fuzdev/tsv',
	author: {
		name: 'Ryan Atkinson',
		email: 'mail@ryanatkn.com',
		url: 'https://www.ryanatkn.com/'
	},
	repository: {
		type: 'git',
		url: 'git+https://github.com/fuzdev/tsv.git'
	},
	bugs: 'https://github.com/fuzdev/tsv/issues',
	funding: 'https://www.ryanatkn.com/funding',
	engines: { node: '>=20' }
};

const write_pkg = (dir: string, pkg: Record<string, unknown>): void => {
	Deno.writeTextFileSync(`${dir}/package.json`, JSON.stringify(pkg, null, '\t') + '\n');
};

// --- the loader package -----------------------------------------------------

const loader_dir = 'crates/tsv_napi/pkg/napi';
Deno.mkdirSync(loader_dir, { recursive: true });
for (const [from, to] of [
	['crates/tsv_napi/npm/index.js', 'index.js'],
	['crates/tsv_napi/npm/index.d.ts', 'index.d.ts'],
	['crates/tsv_napi/npm/README.md', 'README.md'],
	['crates/tsv_wasm/types/tsv_ast.d.ts', 'tsv_ast.d.ts'],
	['LICENSE', 'LICENSE']
]) {
	Deno.copyFileSync(from, `${loader_dir}/${to}`);
}
write_pkg(loader_dir, {
	name: '@fuzdev/tsv',
	version,
	description: 'native formatter and parser for Svelte, TypeScript, and CSS (N-API)',
	// CommonJS loader (no `type: module`) — the native-addon norm; ESM named
	// imports work via Node's CJS interop (see npm/index.js).
	main: 'index.js',
	types: 'index.d.ts',
	exports: {
		'./package.json': './package.json',
		'.': {
			types: './index.d.ts',
			default: './index.js'
		}
	},
	files: ['index.js', 'index.d.ts', 'tsv_ast.d.ts', 'README.md', 'LICENSE'],
	keywords: [
		'typescript',
		'svelte',
		'css',
		'formatter',
		'prettier',
		'parser',
		'ast',
		'acorn',
		'napi',
		'native'
	],
	...shared_metadata,
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
try {
	Deno.copyFileSync(artifact, `${platform_dir}/tsv_napi.node`);
} catch (e) {
	console.error(
		`FAIL: cannot read the built cdylib at ${artifact} — run 'deno task build:napi' first`
	);
	console.error(String(e));
	Deno.exit(1);
}
Deno.copyFileSync('LICENSE', `${platform_dir}/LICENSE`);
Deno.writeTextFileSync(
	`${platform_dir}/README.md`,
	`# @fuzdev/tsv-${triple}\n\n` +
		`> prebuilt tsv N-API binding for ${triple}\n\n` +
		`A platform binary for [\`@fuzdev/tsv\`](https://www.npmjs.com/package/@fuzdev/tsv) — install that package instead; it selects this one automatically.\n`
);
write_pkg(platform_dir, {
	name: `@fuzdev/tsv-${triple}`,
	version,
	description: `prebuilt tsv N-API binding for ${triple}`,
	main: 'tsv_napi.node',
	files: ['tsv_napi.node', 'README.md', 'LICENSE'],
	...platform_fields(triple),
	...shared_metadata
});
const size = Deno.statSync(`${platform_dir}/tsv_napi.node`).size;
console.log(
	`Staged ${platform_dir}: @fuzdev/tsv-${triple} ${version} (tsv_napi.node ${(size / 1024 / 1024).toFixed(1)} MB)`
);
