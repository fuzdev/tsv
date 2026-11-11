/**
 * The N-API size bands over a fake staging: which band grades each binary, what each failure
 * says, that both binaries report before the exit, and where the staging is read.
 */

import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';
import { join } from 'node:path';
import { test } from 'vitest';

import {
	BOUNDS,
	CLI_BOUNDS,
	PGO_BOUNDS,
	PGO_CLI_BOUNDS,
	run_validate_napi_artifact,
	type ValidateNapiArtifactDeps
} from './validate_napi_artifact.ts';

const CHECKOUT = join('/', 'repo');
const PKG = join(CHECKOUT, 'crates/tsv_napi/pkg');

/** The midpoint of a band, which sits inside it. */
const inside = ([min, max]: [number, number]): number => Math.round((min + max) / 2);

interface FakeStaging {
	deps: ValidateNapiArtifactDeps;
	out: Array<string>;
	err: Array<string>;
	/** Every path listed or sized. */
	read: Array<string>;
}

/** A staging whose `pkg/` lists `dirs` and whose files have the sizes `sizes` gives by path
 * relative to `pkg/`; any other file can't be read. */
const fake_staging = (dirs: Array<string>, sizes: Record<string, number>): FakeStaging => {
	const out: Array<string> = [];
	const err: Array<string> = [];
	const read: Array<string> = [];
	return {
		out,
		err,
		read,
		deps: {
			checkout: CHECKOUT,
			list_dirs: async (path) => {
				read.push(path);
				return dirs;
			},
			size: async (path) => {
				read.push(path);
				const size = sizes[path.slice(PKG.length + 1)];
				if (size === undefined) throw new Error(`ENOENT ${path}`);
				return size;
			},
			print: (line) => out.push(line),
			print_error: (line) => err.push(line)
		}
	};
};

const LINUX = 'linux-x64-gnu';

test('the one staged platform passes inside its plain bands, logging both sizes', async () => {
	const addon = inside(BOUNDS[LINUX]!);
	const cli = inside(CLI_BOUNDS[LINUX]!);
	const staging = fake_staging(['napi', LINUX], {
		[`${LINUX}/tsv_napi.node`]: addon,
		[`${LINUX}/tsv`]: cli
	});
	strictEqual(await run_validate_napi_artifact(staging.deps, []), 0);
	deepStrictEqual(staging.err, []);
	strictEqual(staging.out.length, 2);
	ok(staging.out[0]!.startsWith(`OK: ${LINUX} tsv_napi.node ${addon} bytes (`), staging.out[0]);
	ok(staging.out[1]!.startsWith(`OK: ${LINUX} tsv ${cli} bytes (`), staging.out[1]);
	deepStrictEqual(staging.read, [
		PKG,
		join(PKG, `${LINUX}/tsv_napi.node`),
		join(PKG, `${LINUX}/tsv`)
	]);
});

test('--pgo grades against the PGO bands', async () => {
	const sizes = {
		[`${LINUX}/tsv_napi.node`]: inside(BOUNDS[LINUX]!),
		[`${LINUX}/tsv`]: inside(CLI_BOUNDS[LINUX]!)
	};
	const plain = fake_staging([LINUX], sizes);
	strictEqual(await run_validate_napi_artifact(plain.deps, ['--pgo']), 1);
	const [min, max] = PGO_BOUNDS[LINUX]!;
	ok(plain.err[0]!.includes(`outside [${min}, ${max}]`), plain.err[0]);
	const [cli_min, cli_max] = PGO_CLI_BOUNDS[LINUX]!;
	ok(plain.err[1]!.includes(`outside [${cli_min}, ${cli_max}]`), plain.err[1]);

	const pgo = fake_staging([LINUX], {
		[`${LINUX}/tsv_napi.node`]: inside(PGO_BOUNDS[LINUX]!),
		[`${LINUX}/tsv`]: inside(PGO_CLI_BOUNDS[LINUX]!)
	});
	strictEqual(await run_validate_napi_artifact(pgo.deps, ['--pgo']), 0);
});

test('an addon outside its band fails, and the CLI still reports its size', async () => {
	const cli = inside(CLI_BOUNDS[LINUX]!);
	const staging = fake_staging([LINUX], {
		[`${LINUX}/tsv_napi.node`]: 17,
		[`${LINUX}/tsv`]: cli
	});
	strictEqual(await run_validate_napi_artifact(staging.deps, []), 1);
	const [min, max] = BOUNDS[LINUX]!;
	deepStrictEqual(staging.err, [
		`FAIL: ${LINUX} tsv_napi.node is 17 bytes (17 B) — outside [${min}, ${max}]. ` +
			'A deliberate size change must move the bound in scripts/validate_napi_artifact.ts.'
	]);
	strictEqual(staging.out.length, 1);
	ok(staging.out[0]!.startsWith(`OK: ${LINUX} tsv ${cli} bytes`));
});

test('a band holds its bounds and nothing past them', async () => {
	const [min, max] = BOUNDS[LINUX]!;
	const [cli_min, cli_max] = CLI_BOUNDS[LINUX]!;
	const at_bounds = fake_staging([LINUX], {
		[`${LINUX}/tsv_napi.node`]: max,
		[`${LINUX}/tsv`]: cli_min
	});
	strictEqual(await run_validate_napi_artifact(at_bounds.deps, []), 0);
	const past = fake_staging([LINUX], {
		[`${LINUX}/tsv_napi.node`]: max + 1,
		[`${LINUX}/tsv`]: cli_min - 1
	});
	strictEqual(await run_validate_napi_artifact(past.deps, []), 1);
	strictEqual(past.err.length, 2);
	ok(past.err[0]!.includes(`is ${max + 1} bytes`) && past.err[0]!.includes(`[${min}, ${max}]`));
	ok(
		past.err[1]!.includes(`is ${cli_min - 1} bytes`) &&
			past.err[1]!.includes(`[${cli_min}, ${cli_max}]`)
	);
});

test('a binary not staged fails, naming the staging task', async () => {
	const staging = fake_staging([LINUX], { [`${LINUX}/tsv_napi.node`]: inside(BOUNDS[LINUX]!) });
	strictEqual(await run_validate_napi_artifact(staging.deps, []), 1);
	deepStrictEqual(staging.err, [
		`FAIL: crates/tsv_napi/pkg/${LINUX}/tsv not staged — run 'deno task build:napi:packages' first`
	]);
});

test('with no --triple, anything but exactly one platform directory fails', async () => {
	for (const [dirs, found] of [
		[['napi'], 'none'],
		[[], 'none'],
		[['napi', LINUX, 'darwin-arm64'], `${LINUX}, darwin-arm64`]
	] as Array<[Array<string>, string]>) {
		const staging = fake_staging(dirs, {});
		strictEqual(await run_validate_napi_artifact(staging.deps, []), 1);
		deepStrictEqual(staging.err, [
			`FAIL: pass --triple, or stage exactly one platform package (found: ${found})`
		]);
		deepStrictEqual(staging.read, [PKG]);
	}
});

test('--triple names the platform without listing pkg/, and a Windows row grades tsv.exe', async () => {
	const staging = fake_staging([], {
		'win32-x64/tsv_napi.node': inside(PGO_BOUNDS['win32-x64']!),
		'win32-x64/tsv.exe': inside(PGO_CLI_BOUNDS['win32-x64']!)
	});
	strictEqual(
		await run_validate_napi_artifact(staging.deps, ['--triple', 'win32-x64', '--pgo']),
		0
	);
	ok(staging.out[1]!.startsWith('OK: win32-x64 tsv.exe '), staging.out[1]);
	deepStrictEqual(staging.read, [
		join(PKG, 'win32-x64/tsv_napi.node'),
		join(PKG, 'win32-x64/tsv.exe')
	]);
});

test('a plain addon of a row no job builds plain is refused, not passed', async () => {
	const staging = fake_staging([], {
		'darwin-arm64/tsv_napi.node': 4_000_000,
		'darwin-arm64/tsv': inside(CLI_BOUNDS['darwin-arm64']!)
	});
	strictEqual(await run_validate_napi_artifact(staging.deps, ['--triple', 'darwin-arm64']), 1);
	ok(
		staging.err[0]!.startsWith(
			"FAIL: no plain tsv_napi.node band for 'darwin-arm64' — only linux-x64-gnu builds"
		)
	);
	strictEqual(staging.err.length, 1);
	ok(staging.out[0]!.startsWith('OK: darwin-arm64 tsv '));
});

test('a triple with no bands fails for both binaries', async () => {
	for (const argv of [
		['--triple', 'bogus'],
		['--triple', 'bogus', '--pgo']
	]) {
		const staging = fake_staging([], {});
		strictEqual(await run_validate_napi_artifact(staging.deps, argv), 1);
		strictEqual(staging.err.length, 2);
		ok(staging.err[1]!.startsWith("FAIL: no tsv size bounds for triple 'bogus'"), staging.err[1]);
	}
});

test('a flag parseArgs refuses throws before anything is read', async () => {
	const staging = fake_staging([LINUX], {});
	await rejects(run_validate_napi_artifact(staging.deps, ['--bogus']), TypeError);
	deepStrictEqual(staging.read, []);
});
