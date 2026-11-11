/**
 * Whether tsv_debug's sidecar is installed as its lock says — the check the sidecar actor
 * makes before it spawns (`crates/tsv_debug/src/deno/actor.rs`, `check_install`), for the
 * scripts that would otherwise meet its refusal halfway through a run (`doctor`, `publish`).
 *
 * `npm ci` installs exactly the lock, but only when it runs, so a lock changed since (a pin
 * bump pulled, a branch switched) leaves the old tree in place. npm's hidden lockfile
 * (`node_modules/.package-lock.json`) records the tree it installed: the install is current
 * when its `packages` equal the committed lock's, the lock's own root entry (`""`) aside,
 * which the hidden lockfile doesn't carry, and each locked package's installed `package.json`
 * names its lock entry's `version`, so a package directory removed or its version edited
 * since is stale too. An edit to any other file inside the install passes.
 *
 * Pure, so the Deno scripts and the Node ones share it.
 *
 * @module
 */

/** The sidecar's package directory, beneath the checkout. */
export const SIDECAR_DIR = 'crates/tsv_debug/sidecar';

/** The command that installs the sidecar's packages, run from the checkout's root. */
export const SIDECAR_INSTALL_COMMAND = `npm ci --prefix ${SIDECAR_DIR}`;

/** The committed lock, beneath the checkout. */
export const SIDECAR_LOCK_PATH = `${SIDECAR_DIR}/package-lock.json`;

/** npm's hidden lockfile for the sidecar's install, beneath the sidecar's directory. */
const INSTALLED_LOCK_PATH = 'node_modules/.package-lock.json';

/**
 * `current` when the installed tree is the lock's, `missing` with no install, `stale` when
 * the install differs from the lock or its hidden lockfile doesn't parse.
 */
export type SidecarInstallState = 'current' | 'missing' | 'stale';

/** A lockfile's `packages`, its root entry left out, or `null` when it has none. */
const lock_packages = (text: string): Record<string, unknown> | null => {
	let lock: unknown;
	try {
		lock = JSON.parse(text);
	} catch {
		return null;
	}
	if (typeof lock !== 'object' || lock === null || !('packages' in lock)) return null;
	const { packages } = lock;
	if (typeof packages !== 'object' || packages === null) return null;
	const { ['']: _root, ...rest } = packages as Record<string, unknown>;
	return rest;
};

/** Whether two parsed JSON values are equal, object key order aside. */
const json_equal = (a: unknown, b: unknown): boolean => {
	if (a === b) return true;
	if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null) return false;
	if (Array.isArray(a) !== Array.isArray(b)) return false;
	const a_keys = Object.keys(a);
	const b_keys = Object.keys(b);
	return (
		a_keys.length === b_keys.length &&
		a_keys.every(
			(key) =>
				Object.hasOwn(b, key) &&
				json_equal((a as Record<string, unknown>)[key], (b as Record<string, unknown>)[key])
		)
	);
};

/** The `version` in a `package.json`'s text, or `null` when it has none. */
const manifest_version = (text: string): string | null => {
	let manifest: unknown;
	try {
		manifest = JSON.parse(text);
	} catch {
		return null;
	}
	if (typeof manifest !== 'object' || manifest === null || !('version' in manifest)) return null;
	return typeof manifest.version === 'string' ? manifest.version : null;
};

/**
 * The sidecar install's state, from the committed lock's text and the install's files.
 * @param committed_lock - the text of the committed `package-lock.json`
 * @param read_installed - a file's text by its path beneath the sidecar's directory
 * (`node_modules/…`), `null` when it doesn't exist
 * @throws `Error` - when the committed lock has no `packages`, a broken checkout rather than
 * an install to fix
 */
export const sidecar_install_state = (
	committed_lock: string,
	read_installed: (path: string) => string | null
): SidecarInstallState => {
	const committed = lock_packages(committed_lock);
	if (committed === null) throw new Error(`${SIDECAR_LOCK_PATH} has no \`packages\` object`);
	const installed_lock = read_installed(INSTALLED_LOCK_PATH);
	if (installed_lock === null) return 'missing';
	const installed = lock_packages(installed_lock);
	if (installed === null || !json_equal(committed, installed)) return 'stale';
	for (const [key, entry] of Object.entries(committed)) {
		const locked =
			typeof entry === 'object' && entry !== null && 'version' in entry ? entry.version : null;
		const manifest = read_installed(`${key}/package.json`);
		if (typeof locked !== 'string' || manifest === null || manifest_version(manifest) !== locked) {
			return 'stale';
		}
	}
	return 'current';
};
