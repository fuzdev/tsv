/**
 * `stage_file`: the copy, then the copy dated at the staging, never the source.
 */

import { deepStrictEqual, ok, rejects, strictEqual } from 'node:assert';
import { test } from 'vitest';

import { stage_file, type StageFileDeps, type StageTimes } from './stage_file.ts';

const recording_deps = (): { deps: StageFileDeps; calls: Array<Array<unknown>> } => {
	const calls: Array<Array<unknown>> = [];
	return {
		calls,
		deps: {
			copy: async (from, to) => {
				calls.push(['copy', from, to]);
			},
			set_times: async (path, times) => {
				calls.push(['set_times', path, times]);
			}
		}
	};
};

test('copies, then dates the copy at now', async () => {
	const { deps, calls } = recording_deps();
	const before = Date.now();
	await stage_file(deps, '/src/a.js', '/pkg/a.js');
	const after = Date.now();
	strictEqual(calls.length, 2);
	deepStrictEqual(calls[0], ['copy', '/src/a.js', '/pkg/a.js']);
	const [kind, path, times] = calls[1]! as [string, string, StageTimes];
	strictEqual(kind, 'set_times');
	strictEqual(path, '/pkg/a.js');
	ok(times.mtime_ms >= before && times.mtime_ms <= after, `mtime ${times.mtime_ms}`);
	strictEqual(times.atime_ms, times.mtime_ms);
});

test('a failed copy dates nothing', async () => {
	const { deps, calls } = recording_deps();
	deps.copy = async () => {
		throw new Error('ENOENT');
	};
	await rejects(stage_file(deps, '/src/absent', '/pkg/absent'), /ENOENT/);
	deepStrictEqual(calls, []);
});
