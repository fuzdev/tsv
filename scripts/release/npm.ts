/**
 * The npm registry questions a release asks.
 *
 * @module
 */

import { command_capture, type Result } from './command.ts';

/** The logged-in npm user. */
export const npm_whoami = async (): Promise<Result<{ user: string }>> => {
	const who = await command_capture('npm', ['whoami']);
	if (who.success) return { ok: true, user: who.out };
	return { ok: false, error: who.err.split('\n')[0] ?? '' };
};

/**
 * Whether `name@version` is on the registry. `npm view` prints the version when it is, nothing
 * when the package exists without it, and E404 when the package does not exist; anything else
 * is a lookup failure, which no publish decision may be made without.
 */
export const npm_is_published = async (
	name: string,
	version: string
): Promise<Result<{ published: boolean }>> => {
	const view = await command_capture('npm', ['view', `${name}@${version}`, 'version']);
	if (view.success) return { ok: true, published: view.out === version };
	if (view.err.includes('E404')) return { ok: true, published: false };
	return { ok: false, error: `npm view ${name}@${version} failed:\n${view.err}` };
};

/** Whether a failed `npm publish --dry-run` failed only because the version is already on the
 * registry, which a dry run of a release that has not bumped yet always is. */
export const npm_error_is_republish = (stderr: string): boolean =>
	/cannot publish over the previously published versions?/i.test(stderr);
