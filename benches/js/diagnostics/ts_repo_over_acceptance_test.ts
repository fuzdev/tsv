/**
 * `run_ts_repo_over_acceptance` over fake readers and fake engines: the cache's three refusals
 * (absent, a moved count, an unreadable listed file), the profile's rows and `--json` report,
 * and the tsc row that must read zero. Package-free, so it gates in `npm run test:node`.
 */

import { deepStrictEqual, ok, strictEqual } from 'node:assert';
import { afterEach, beforeEach, test, vi } from 'vitest';

import { TS_REPO_REJECTS_PIN } from '../lib/gate_counts.ts';
import type { TsvImplementation } from '../lib/types.ts';
import {
	run_ts_repo_over_acceptance,
	TS_REPO_REJECTS_PATH,
	type TsRepoOverAcceptanceDeps,
	type TsRepoOverAcceptanceImplementations
} from './ts_repo_over_acceptance.ts';

let err: string[];
beforeEach(() => {
	err = [];
	vi.spyOn(console, 'error').mockImplementation((...a: unknown[]) => void err.push(a.join(' ')));
});
afterEach(() => {
	vi.restoreAllMocks();
});

const PATHS = Array.from(
	{ length: TS_REPO_REJECTS_PIN },
	(_, i) => `../typescript/tests/cases/${i % 7 === 0 ? 'accept' : 'reject'}_${i}.ts`
);
const ACCEPTED = PATHS.filter((p) => p.includes('/accept_')).length;

/** An engine that accepts every source (`all`), none, or those holding `accept` (`some`). */
const fake_impl = (
	disposed: string[],
	name: string,
	accepts: 'all' | 'none' | 'some'
): TsvImplementation =>
	({
		supports_parse_language: (language: string) => language === 'typescript',
		parse: (source: string) => {
			if (accepts === 'none' || (accepts === 'some' && !source.includes('accept'))) {
				throw new Error('rejected');
			}
			return {};
		},
		dispose: () => void disposed.push(name)
	}) as unknown as TsvImplementation;

const fake_deps = (
	options: {
		cache?: string | null;
		read?: (path: string) => Promise<string>;
		impls?: (disposed: string[]) => TsRepoOverAcceptanceImplementations;
	} = {}
) => {
	const state = { inits: 0, disposed: [] as string[], out: '' };
	const deps: TsRepoOverAcceptanceDeps = {
		checkout: {
			read_text: async (path) => {
				strictEqual(path, TS_REPO_REJECTS_PATH);
				const cache = options.cache === undefined ? JSON.stringify(PATHS) : options.cache;
				if (cache === null) throw new Error('not_found');
				return cache;
			}
		},
		typescript: { read_text: options.read ?? (async (path) => `// ${path}`) },
		init_implementations: async () => {
			state.inits++;
			return (
				options.impls?.(state.disposed) ?? {
					tsc: fake_impl(state.disposed, 'tsc', 'none'),
					canonical: fake_impl(state.disposed, 'canonical', 'none'),
					native: fake_impl(state.disposed, 'native', 'some'),
					wasm: undefined
				}
			);
		},
		stdout: { write: async (data) => void (state.out += data) }
	};
	return { deps, state };
};

test('an absent cache fails before any engine loads', async () => {
	const { deps, state } = fake_deps({ cache: null });
	strictEqual(await run_ts_repo_over_acceptance(deps, []), 1);
	strictEqual(state.inits, 0);
	ok(err.some((l) => l.startsWith(`FAIL: ${TS_REPO_REJECTS_PATH} not found`)));
});

test('a cache whose count moved from the pin fails', async () => {
	const { deps, state } = fake_deps({ cache: JSON.stringify(PATHS.slice(1)) });
	strictEqual(await run_ts_repo_over_acceptance(deps, []), 1);
	strictEqual(state.inits, 0);
	ok(err.some((l) => l.includes(`cache holds ${PATHS.length - 1} rejects ≠ pinned`)));
});

test('a listed file that cannot be read fails, naming it', async () => {
	const { deps } = fake_deps({
		read: async (path) => {
			if (path.endsWith('_5.ts')) throw new Error('not_found');
			return '';
		}
	});
	strictEqual(await run_ts_repo_over_acceptance(deps, []), 1);
	ok(
		err.includes(
			'FAIL: cannot read ../typescript/tests/cases/reject_5.ts — the ../typescript checkout moved under the cache.'
		)
	);
});

test('a listed path outside ../typescript is refused, never read', async () => {
	const read: string[] = [];
	const { deps } = fake_deps({
		cache: JSON.stringify(['/etc/passwd', ...PATHS.slice(1)]),
		read: async (path) => {
			read.push(path);
			return '';
		}
	});
	strictEqual(await run_ts_repo_over_acceptance(deps, []), 1);
	deepStrictEqual(read, []);
	ok(
		err.includes(
			'FAIL: cannot read /etc/passwd — the ../typescript checkout moved under the cache.'
		)
	);
});

test('the profile reports each available row, reads paths beneath ../typescript, and disposes', async () => {
	const read: string[] = [];
	const { deps, state } = fake_deps({
		read: async (path) => {
			read.push(path);
			return `// ${path}`;
		}
	});
	strictEqual(await run_ts_repo_over_acceptance(deps, ['--json']), 0);
	strictEqual(read[0], 'tests/cases/accept_0.ts');
	const report = JSON.parse(state.out);
	strictEqual(report.files, TS_REPO_REJECTS_PIN);
	strictEqual(report.oracle_agrees, true);
	deepStrictEqual(
		report.rows.map((r: { name: string; accepted: number }) => [r.name, r.accepted]),
		[
			['tsc', 0],
			['acorn-typescript', 0],
			['tsv', ACCEPTED]
		]
	);
	deepStrictEqual(state.disposed.toSorted(), ['canonical', 'native', 'tsc']);
});

test('the tsc row reading nonzero fails the run', async () => {
	const { deps, state } = fake_deps({
		impls: (disposed) => ({
			tsc: fake_impl(disposed, 'tsc', 'some'),
			canonical: fake_impl(disposed, 'canonical', 'none')
		})
	});
	strictEqual(await run_ts_repo_over_acceptance(deps, ['--json']), 1);
	strictEqual(JSON.parse(state.out).oracle_agrees, false);
	ok(err.some((l) => l.startsWith(`FAIL: the tsc row accepted ${ACCEPTED} of its own rejects`)));
});

test('a tsc that rejects every file passes, and the human report ranks the rows', async () => {
	const { deps, state } = fake_deps({
		impls: (disposed) => ({
			tsc: fake_impl(disposed, 'tsc', 'none'),
			canonical: fake_impl(disposed, 'canonical', 'all'),
			native: fake_impl(disposed, 'native', 'some')
		})
	});
	strictEqual(await run_ts_repo_over_acceptance(deps, ['-v']), 0);
	strictEqual(state.out, '');
	const rows = err.filter((l) => /^ {2}\S/.test(l) && l.includes('/'));
	deepStrictEqual(
		rows.map((l) => l.trim().split(/\s+/)[0]),
		['tsc', 'tsv', 'acorn-typescript']
	);
	ok(err.some((l) => l === '      ../typescript/tests/cases/accept_0.ts'));
	ok(err.includes(`  ⚠ skipping tsv-wasm — not available on this machine`));
});
