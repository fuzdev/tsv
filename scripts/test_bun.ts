/**
 * The parse-failure table (`scripts/syntax_error_suite.ts`) over the staged npm packages
 * under BUN, for both engines — the one runtime that disagrees with the contract's
 * premise. Bun gives every `Error` its own non-enumerable numeric `line` and `column`, a
 * plain set writes through them and keeps them non-enumerable, and the facade's
 * `call_engine` converts only an own ENUMERABLE point into the `SyntaxError` — so each
 * engine deletes the three properties before it sets them (`parse_err` in
 * `crates/tsv_wasm/src/lib.rs`, `pointed_error` in `crates/tsv_napi/src/lib.rs`). Under
 * Node and Deno that delete is a no-op, and every other package suite runs under Node:
 * this is the one place either engine's delete is graded.
 *
 * Narrow by design. The package suites themselves run under `bun test` nearly whole, but
 * their CLI and resolution rows assert Node's own behavior; this registers the shared
 * table alone, its rows and assertions unforked. Each package is imported the way a Bun
 * consumer imports it — by bare specifier, through a temp `node_modules` — so the entry
 * Bun's export conditions pick is the one graded (the auto-init `node` entry; the lazy
 * `default` one would throw `WASM not initialized` on the first row).
 *
 * One file, two roles, split on `process.versions.bun`: run by Deno (`deno task
 * test:bun`) it is the launcher — it names the packages, spawns `bun test` on itself,
 * and warn-skips, exiting 0, where no `bun` is on the PATH — or, with `--require-bun`
 * (what a publish `--wetrun` passes), fails; run by `bun test` it is the suite. Positional
 * arguments (`format`, `parse`, `all`, `napi`) name exactly the packages graded, each
 * required; none grades all four. A missing or stale staging fails like the package
 * suites' (`scripts/check_staged_freshness.ts`, `BENCH_STALE_OK=1` the same override).
 *
 * Usage: deno task test:bun [--require-bun] [format|parse|all|napi ...]
 * Prerequisite: the named stagings — deno task build:npm:<variant>, build:napi:packages
 */

import { after } from 'node:test';
import { spawnSync } from 'node:child_process';
import {
	cpSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	rmSync,
	symlinkSync,
	writeFileSync
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { argv, env, exit, versions } from 'node:process';
import { fileURLToPath, pathToFileURL } from 'node:url';

import {
	assert_staged_fresh,
	NAPI_LOADER_DIR,
	NAPI_PKG_ROOT,
	napi_addon_check,
	napi_loader_checks,
	napi_staged_triple,
	wasm_package_checks,
	wasm_package_dir
} from './check_staged_freshness.ts';
import { register_syntax_error_suite } from './syntax_error_suite.ts';

const PACKAGES = ['format', 'parse', 'all', 'napi'] as const;
type Package = (typeof PACKAGES)[number];

/** The launcher flag that turns a missing `bun` from a warn-skip into a failure. */
const REQUIRE_FLAG = '--require-bun';

/** The env var the launcher hands the suite its package list in. */
const PACKAGES_ENV = 'TSV_BUN_PACKAGES';

const ROOT = fileURLToPath(new URL('..', import.meta.url));

if (versions.bun === undefined) {
	launch(argv.slice(2));
} else {
	await register(env[PACKAGES_ENV]?.split(',') ?? []);
}

/** The launcher: validate the package names, then run this file under `bun test`. */
function launch(args: Array<string>): void {
	const require_bun = args.includes(REQUIRE_FLAG);
	const names = [...new Set(args.filter((arg) => arg !== REQUIRE_FLAG))];
	const unknown = names.filter((arg) => !(PACKAGES as ReadonlyArray<string>).includes(arg));
	if (unknown.length > 0) {
		console.error(
			`unknown argument: ${unknown.join(', ')} (expected ${REQUIRE_FLAG} and packages from ${PACKAGES.join(' / ')})`
		);
		exit(1);
	}
	const packages = names.length > 0 ? names : PACKAGES;
	const result = spawnSync('bun', ['test', fileURLToPath(import.meta.url)], {
		stdio: 'inherit',
		env: { ...env, [PACKAGES_ENV]: packages.join(',') }
	});
	if ((result.error as { code?: string } | undefined)?.code === 'ENOENT') {
		const what = `the parse-failure table under Bun over ${packages.join(', ')}`;
		if (require_bun) {
			console.error(`✗ bun not found — ${what} is required here (${REQUIRE_FLAG}); install bun`);
			exit(1);
		}
		console.warn(
			`⚠ bun not found — SKIPPED ${what}; ` +
				"neither engine's Bun `line` / `column` delete is graded on this machine"
		);
		exit(0);
	}
	if (result.error) throw result.error;
	if (result.status === null) {
		console.error(`✗ bun test died by signal ${result.signal}`);
		exit(1);
	}
	exit(result.status);
}

/** The suite: stage each named package into a temp consumer and register the table over it. */
async function register(packages: Array<string>): Promise<void> {
	if (
		packages.length === 0 ||
		packages.some((p) => !(PACKAGES as ReadonlyArray<string>).includes(p))
	) {
		console.error(
			`${PACKAGES_ENV} must name packages from ${PACKAGES.join(' / ')} — run through the launcher: deno task test:bun`
		);
		exit(1);
	}
	const named = packages as Array<Package>;
	const triple = named.includes('napi') ? napi_staged_triple() : '';
	await assert_staged_fresh(
		named.flatMap((p) =>
			p === 'napi' ? [napi_addon_check(triple), ...napi_loader_checks()] : wasm_package_checks(p)
		)
	);

	const consumer = mkdtempSync(join(tmpdir(), 'tsv_bun_'));
	after(() => {
		// best-effort: a leaked temp staging is harmless, and a failed cleanup must not fail
		// an otherwise-green run
		try {
			rmSync(consumer, { recursive: true, force: true });
		} catch {
			// leaked
		}
	});
	const scope = join(consumer, 'node_modules', '@fuzdev');
	mkdirSync(scope, { recursive: true });

	// every import ahead of the first `describe`: a top-level await after one yields to a
	// runner that may already be draining what is registered
	const apis: Array<[string, Record<string, unknown>]> = [];
	for (const p of named) {
		let name: string;
		if (p === 'napi') {
			// copies, not links: the loader resolves `@fuzdev/tsv-<triple>` from its REAL path
			name = '@fuzdev/tsv';
			cpSync(join(ROOT, NAPI_LOADER_DIR), join(scope, 'tsv'), { recursive: true });
			cpSync(join(ROOT, NAPI_PKG_ROOT, triple), join(scope, `tsv-${triple}`), { recursive: true });
		} else {
			// a link: the wasm packages import nothing but their own relative files
			const dir = join(ROOT, wasm_package_dir(p));
			name = JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8')).name;
			symlinkSync(dir, join(consumer, 'node_modules', name), 'junction');
		}
		// the bare specifier, resolved from inside the consumer as its own import would be
		const entry = join(consumer, `${p}.mjs`);
		writeFileSync(entry, `export * from ${JSON.stringify(name)};\n`);
		apis.push([name, await import(pathToFileURL(entry).href)]);
	}
	for (const [name, api] of apis) register_syntax_error_suite(`${name} under Bun`, api);
}
