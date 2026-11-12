/**
 * The staged-package checks (`check_staged_freshness.ts`) graded over a temp checkout: each
 * check finds its staging fresh when every source is older, and stale when any one input this
 * suite names for it is newer, so a check that stopped reading an input fails here. The inputs
 * are each staging's own: what wasm-pack builds the bundle from, the patcher and its modules
 * (`WASM_STAGING_SOURCES`), the N-API staging and its modules (`NAPI_STAGING_SOURCES`), and the
 * files each staging copies. Then the reads that fail: a missing staged file, a check naming a
 * source that isn't there, and a source that is there but can't be read.
 */

import { ok, rejects, strictEqual } from 'node:assert';
import { mkdir, mkdtemp, rm, symlink, utimes, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { test } from 'vitest';

import { node_source_tree } from '../benches/js/lib/check_artifact_freshness.ts';
import { CORE_CRATES, WASM_CRATES, type WasmVariant } from '../benches/js/lib/tsv_artifacts.ts';
import {
	NAPI_STAGING_SOURCES,
	napi_addon_check,
	napi_loader_checks,
	type StagedCheck,
	staged_staleness,
	WASM_STAGING_SOURCES,
	wasm_package_checks
} from './check_staged_freshness.ts';
import { ALL_FAMILIES, facade_sources } from './npm_facade.ts';
import { THIRD_PARTY_NOTICE } from './npm_metadata.ts';

/** Minutes apart, so no comparison ties. */
const T_SOURCE = Date.UTC(2026, 0, 1);
const T_STAGED = T_SOURCE + 60_000;
const T_LATER = T_STAGED + 60_000;

const TRIPLE = 'linux-x64-gnu';
const WORKSPACE = ['Cargo.toml', 'Cargo.lock'];
/** A crate's source as the suite writes it, and the label a walk gives it. */
const crate_source = (crate: string): string => `crates/${crate}/src/lib.rs`;
const label_of = (path: string): string =>
	path.startsWith('crates/') && path.includes('/src/') ? path.slice('crates/'.length) : path;

/** One check and the inputs that must each stale it. */
interface Case {
	check: StagedCheck;
	inputs: Array<string>;
}

const crate_inputs = (crates: Array<string>): Array<string> => [
	...crates.map(crate_source),
	...WORKSPACE
];

const wasm_cases = (variant: WasmVariant): Array<Case> => {
	const checks = new Map(wasm_package_checks(variant).map((c) => [c.label, c]));
	const take = (label: string): StagedCheck => {
		const check = checks.get(label);
		ok(check, `no check labeled ${label}`);
		return check;
	};
	const wasm_crates = [...CORE_CRATES, ...WASM_CRATES];
	return [
		{
			check: take(`staged WASM bundle (${variant})`),
			inputs: [...crate_inputs(wasm_crates), 'package.json']
		},
		{
			check: take(`staged package entries (${variant})`),
			inputs: [
				...crate_inputs(wasm_crates),
				'package.json',
				...WASM_STAGING_SOURCES,
				THIRD_PARTY_NOTICE.source
			]
		},
		{
			check: take(`staged facade (${variant})`),
			inputs: [
				...facade_sources({
					format: variant !== 'parse',
					parse: variant !== 'format',
					strip: variant === 'all',
					imports: variant === 'all'
				}),
				'scripts/npm_facade.ts'
			]
		},
		...(variant === 'all'
			? [{ check: take('staged cli.js (all)'), inputs: ['crates/tsv_wasm/npm/cli.js'] }]
			: [])
	];
};

const napi_cases = (): Array<Case> => {
	const [loader, cli] = napi_loader_checks();
	ok(loader && cli);
	return [
		{
			check: napi_addon_check(TRIPLE),
			inputs: [
				...crate_inputs([...CORE_CRATES, 'tsv_napi', 'tsv_ignore', 'tsv_discover']),
				...NAPI_STAGING_SOURCES,
				THIRD_PARTY_NOTICE.source
			]
		},
		{
			check: loader,
			inputs: [
				'crates/tsv_napi/npm/index.js',
				'crates/tsv_napi/npm/index.d.ts',
				'crates/tsv_napi/npm/platform.js',
				'crates/tsv_napi/npm/bin.js',
				...facade_sources(ALL_FAMILIES),
				...NAPI_STAGING_SOURCES,
				THIRD_PARTY_NOTICE.source
			]
		},
		{ check: cli, inputs: ['crates/tsv_wasm/npm/cli.js'] }
	];
};

const CASES: Array<[name: string, cases: Array<Case>]> = [
	['the format package', wasm_cases('format')],
	['the parse package', wasm_cases('parse')],
	['the all package', wasm_cases('all')],
	['the N-API packages', napi_cases()]
];

/** A temp checkout holding each case's inputs and the files its check names, dated `T_SOURCE`,
 * and each staged file dated `T_STAGED`; its path ends in a separator. */
const make_checkout = async (cases: Array<Case>): Promise<string> => {
	const root = (await mkdtemp(join(tmpdir(), 'tsv_check_staged_freshness_test_'))) + '/';
	const write = async (path: string, ms: number): Promise<void> => {
		await mkdir(dirname(`${root}${path}`), { recursive: true });
		await writeFile(`${root}${path}`, path);
		await date(root, path, ms);
	};
	for (const { check, inputs } of cases) {
		for (const path of [...inputs, ...check.files]) await write(path, T_SOURCE);
		await write(check.staged, T_STAGED);
	}
	return root;
};

const date = (root: string, path: string, ms: number): Promise<void> =>
	utimes(`${root}${path}`, ms / 1000, ms / 1000);

/** The check graded over a fresh tree, so no memoized walk carries over. */
const grade = (root: string, check: StagedCheck) => staged_staleness(check, node_source_tree(root));

for (const [name, cases] of CASES) {
	test(`${name}: fresh when every source is older, stale when any input is newer`, async () => {
		const root = await make_checkout(cases);
		try {
			for (const { check, inputs } of cases) {
				strictEqual(await grade(root, check), undefined, check.label);
				for (const input of inputs) {
					await date(root, input, T_LATER);
					const staleness = await grade(root, check);
					ok(staleness, `${check.label} is fresh with ${input} newer`);
					strictEqual(staleness.missing, false);
					ok(
						staleness.reason.startsWith(`${check.label}: staged `) &&
							staleness.reason.includes(`, but ${label_of(input)} changed `),
						staleness.reason
					);
					await date(root, input, T_SOURCE);
				}
			}
		} finally {
			await rm(root, { recursive: true });
		}
	});
}

test('a missing staged file is missing, and a named source that is not there throws', async () => {
	const [bundle] = wasm_cases('format');
	ok(bundle);
	const root = await make_checkout([bundle]);
	try {
		await rm(`${root}${bundle.check.staged}`);
		const staleness = await grade(root, bundle.check);
		ok(staleness);
		strictEqual(staleness.missing, true);
		strictEqual(staleness.reason, `${bundle.check.label}: not staged — ${bundle.check.staged}`);
		await rm(`${root}package.json`);
		await rejects(grade(root, { ...bundle.check, staged: 'Cargo.toml' }), {
			message: `${bundle.check.label}: named source package.json does not exist — fix the check`
		});
	} finally {
		await rm(root, { recursive: true });
	}
});

test("a source that is there but can't be read throws, naming it", async () => {
	const [bundle] = wasm_cases('format');
	ok(bundle);
	const root = await make_checkout([bundle]);
	try {
		// a link to itself: there, and unreadable whoever runs the suite
		await symlink('loop.rs', `${root}crates/tsv_wasm/src/loop.rs`);
		await rejects(grade(root, bundle.check), (error: Error) =>
			error.message.startsWith("can't read crates/tsv_wasm/src/loop.rs to date what it builds: ")
		);
	} finally {
		await rm(root, { recursive: true });
	}
});
