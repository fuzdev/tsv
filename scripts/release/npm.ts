/**
 * The npm registry questions a release asks, each run in `cwd` with the terminal's stdin, as
 * npm's auth may prompt there.
 *
 * @module
 */

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

import { program_capture } from './program.ts';
import type { Result } from './result.ts';

/** `npm`, or `null` when there's none, and where it runs. */
export interface ReleaseNpm {
	program: SpawnProgram | null;
	cwd: FsReadDir;
}

const npm_capture = (npm: ReleaseNpm, args: ReadonlyArray<string>) =>
	program_capture(npm.program, 'npm', args, { cwd: npm.cwd, stdin: 'inherit' });

/**
 * Passed to `npm whoami` and `npm view`, whose stdout is read as text: npm's `json` setting,
 * from an npmrc or `npm_config_json`, prints either as JSON (`"1.2.3"`), and the command line
 * outranks every other place npm reads it from. `npm config get` prints its value raw either
 * way.
 */
const NO_JSON = '--json=false';

/** The logged-in npm user. */
export const npm_whoami = async (npm: ReleaseNpm): Promise<Result<{ user: string }>> => {
	const who = await npm_capture(npm, ['whoami', NO_JSON]);
	if (who.success) return { ok: true, user: who.out };
	return { ok: false, error: who.err.split('\n')[0] ?? '' };
};

/**
 * Whether `name@version` is on the registry. `npm view` prints the version when it is, and
 * E404 when the package or the version is absent (an exit-0 answer naming anything else is
 * read as absent too); anything else is a lookup failure, which no publish decision may be
 * made without.
 */
export const npm_is_published = async (
	npm: ReleaseNpm,
	name: string,
	version: string
): Promise<Result<{ published: boolean }>> => {
	const view = await npm_capture(npm, ['view', `${name}@${version}`, 'version', NO_JSON]);
	if (view.success) return { ok: true, published: view.out === version };
	if (view.err.includes('E404')) return { ok: true, published: false };
	return { ok: false, error: `npm view ${name}@${version} failed:\n${view.err}` };
};

/**
 * npm's `dry-run` setting, as `npm config get dry-run` prints it: from the environment
 * (`npm_config_dry_run`, a name npm reads in any case and with `-` for `_`, which `npm run …
 * --dry-run` sets for the script it runs) or an npmrc. npm prints the raw value, and `npm
 * publish` takes most strings as on (`1`, `yes`, `FALSE`), so a release takes only `false` as
 * off.
 */
export const npm_dry_run_setting = async (npm: ReleaseNpm): Promise<Result<{ value: string }>> => {
	const got = await npm_capture(npm, ['config', 'get', 'dry-run']);
	if (got.success) return { ok: true, value: got.out.trim() };
	return { ok: false, error: `npm config get dry-run failed: ${got.err.split('\n')[0] ?? ''}` };
};

/** Whether a failed `npm publish --dry-run` failed only because the version is already on the
 * registry, which a dry run of a release that has not bumped yet always is. */
export const npm_error_is_republish = (stderr: string): boolean =>
	/cannot publish over the previously published versions?/i.test(stderr);
