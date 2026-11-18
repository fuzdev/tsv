/**
 * `/`-separated paths as text, read lexically as `node:path`'s POSIX functions read them, for
 * the gro-hosted scripts, which reach the filesystem only through gro's handles and so need
 * `node:path` for nothing but its string work. Nothing here touches the filesystem: a `..`
 * cancels the segment before it whatever that segment names, a symlink included.
 *
 * @module
 */

/**
 * `path` with `.` segments and repeated slashes dropped and each `..` cancelling the segment
 * before it, as `node:path`'s `posix.normalize`: an absolute path never climbs above `/`, a
 * relative one keeps the `..` segments it can't cancel, a trailing slash is kept, and an empty
 * result is `.` (`./` with a trailing slash).
 */
export const normalize_path = (path: string): string => {
	if (path === '') return '.';
	const absolute = path.startsWith('/');
	const trailing = path.endsWith('/');
	const segments: Array<string> = [];
	for (const segment of path.split('/')) {
		if (segment === '' || segment === '.') continue;
		if (segment !== '..') segments.push(segment);
		else if (segments.length > 0 && segments.at(-1) !== '..') segments.pop();
		else if (!absolute) segments.push('..');
	}
	const joined = segments.join('/');
	if (joined === '') return absolute ? '/' : trailing ? './' : '.';
	return (absolute ? '/' : '') + joined + (trailing ? '/' : '');
};

/** The non-empty `parts` joined by `/` and normalized (`normalize_path`), or `.` when every
 * one is empty, as `node:path`'s `posix.join`. */
export const join_path = (...parts: ReadonlyArray<string>): string => {
	const joined = parts.filter((part) => part !== '').join('/');
	return joined === '' ? '.' : normalize_path(joined);
};

/**
 * `path` against the absolute directory `base`, normalized and without a trailing slash, as
 * `node:path`'s `posix.resolve(base, path)`: an absolute `path` stands alone.
 * @throws `TypeError` - when `base` is not absolute, which `posix.resolve` would read against
 *   the working directory
 */
export const resolve_path = (base: string, path: string): string => {
	if (!base.startsWith('/')) throw new TypeError(`resolve_path takes an absolute base: ${base}`);
	const resolved = normalize_path(path.startsWith('/') ? path : `${base}/${path}`);
	return resolved.length > 1 && resolved.endsWith('/') ? resolved.slice(0, -1) : resolved;
};
