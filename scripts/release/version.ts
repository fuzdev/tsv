/**
 * The release version: where it is declared (`Cargo.toml`'s `[workspace.package]` table, which
 * wasm-pack stamps into the npm package) and how a bump moves it. Pure text helpers, no I/O, so
 * the reader and the writer share one grammar.
 *
 * @module
 */

import type { Result } from './command.ts';

export const CARGO_PATH = 'Cargo.toml';

export const BUMP_LEVELS = ['patch', 'minor', 'major'] as const;
export type BumpLevel = (typeof BUMP_LEVELS)[number];

export const is_bump_level = (value: unknown): value is BumpLevel =>
	(BUMP_LEVELS as ReadonlyArray<unknown>).includes(value);

/** The `version` key of the `[workspace.package]` table. `[^[]*?` bounds the scan to the table
 * (it stops at the next `[` heading), so a missing key can never land a rewrite on a later
 * table's line, and `^version` (multiline) skips `rust-version`. */
const WORKSPACE_VERSION = /(\[workspace\.package\][^[]*?^version\s*=\s*)"([^"]*)"/m;

/** The `[workspace.package]` version in a `Cargo.toml`'s text, or null when it declares none. */
export const cargo_workspace_version = (cargo_toml: string): string | null =>
	WORKSPACE_VERSION.exec(cargo_toml)?.[2] ?? null;

/** The `[workspace.dependencies]` table's body: every line up to the next `[` heading. */
const WORKSPACE_DEPENDENCIES = /(^\[workspace\.dependencies\]\n)((?:(?!\[).*\n?)*)/m;

/** An exact `version = "=x.y.z"` requirement inside one entry's inline table — the whole
 * requirement, so a compound one (`"=0.7.1, <1"`) is not a pin the bump can move. */
const EXACT_VERSION = /(\bversion\s*=\s*")=\s*\d+\.\d+\.\d+(")/;

/** Whether a `[workspace.dependencies]` line is a workspace member's entry: an inline table
 * naming a `path` (a registry dependency has none, so its own requirement is never touched). */
const is_path_entry = (line: string): boolean =>
	/^[ \t]*(?:[\w-]+|"[^"]*")\s*=\s*\{[^#]*\bpath\s*=/.test(line);

/** A `[workspace.dependencies.<name>]` table heading: an entry the line reader below cannot see. */
const WORKSPACE_DEPENDENCY_TABLE = /^[ \t]*\[[ \t]*workspace\.dependencies\.[^\]]*\]/m;

/** Any `version` key inside one entry's inline table, however its requirement is spelled (read
 * ahead of a trailing `#` comment, which no requirement or crate path here contains). */
const ANY_VERSION = /\bversion\s*=/;

/** The `Cargo.toml` text with its `[workspace.package]` version replaced, along with every exact
 * `=x.y.z` pin on a `[workspace.dependencies]` path entry, so the sibling requirements move in
 * the same write as the version they name (cargo refuses a path dependency whose pin its own
 * version does not meet). Every other byte is kept, and setting the version a text already holds
 * returns it unchanged (idempotent).
 *
 * Refuses, rather than writing a text that still names the old version, when there is no
 * workspace version or a path entry's requirement is one the setter cannot rewrite: not an exact
 * double-quoted `"=x.y.z"` pin, or an entry declared as its own `[workspace.dependencies.<name>]`
 * table. Either would leave a sibling pinned to the old version, which the release's
 * `cargo update --workspace` refuses only after the bump is written. */
export const cargo_workspace_version_set = (
	cargo_toml: string,
	version: string
): Result<{ text: string }> => {
	if (!WORKSPACE_VERSION.test(cargo_toml))
		return { ok: false, error: `no [workspace.package] version in ${CARGO_PATH}` };
	const table = WORKSPACE_DEPENDENCY_TABLE.exec(cargo_toml);
	if (table) {
		return {
			ok: false,
			error: `${CARGO_PATH} declares ${table[0]} — the version bump rewrites inline [workspace.dependencies] entries only`
		};
	}
	const unpinned: Array<string> = [];
	const text = cargo_toml
		.replace(WORKSPACE_VERSION, `$1"${version}"`)
		.replace(
			WORKSPACE_DEPENDENCIES,
			(_, heading: string, body: string) =>
				heading +
				body
					.split('\n')
					.map((line) => {
						if (!is_path_entry(line)) return line;
						if (EXACT_VERSION.test(line)) return line.replace(EXACT_VERSION, `$1=${version}$2`);
						if (ANY_VERSION.test(line.split('#')[0]!)) unpinned.push(line.trim());
						return line;
					})
					.join('\n')
		);
	if (unpinned.length > 0) {
		return {
			ok: false,
			error: `${CARGO_PATH} has a [workspace.dependencies] path entry whose version is not an exact "=x.y.z" pin the bump can move:\n${unpinned.map((line) => `    ${line}`).join('\n')}`
		};
	}
	return { ok: true, text };
};

/** A release version: `<major>.<minor>.<patch>`, no prerelease or build suffix. */
export interface ReleaseVersion {
	major: number;
	minor: number;
	patch: number;
}

/** Parse a `<major>.<minor>.<patch>` version, or null for anything else (a prerelease included,
 * which `--bump` has no level for). Leading zeros are refused, as semver refuses them. */
export const version_parse = (version: string): ReleaseVersion | null => {
	const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.exec(version);
	if (!match) return null;
	return { major: Number(match[1]), minor: Number(match[2]), patch: Number(match[3]) };
};

/** The version one `level` past `version`, or null when `version` is not a release version. A
 * bump resets every lower component. */
export const version_bump = (version: string, level: BumpLevel): string | null => {
	const v = version_parse(version);
	if (!v) return null;
	switch (level) {
		case 'major':
			return `${v.major + 1}.0.0`;
		case 'minor':
			return `${v.major}.${v.minor + 1}.0`;
		case 'patch':
			return `${v.major}.${v.minor}.${v.patch + 1}`;
	}
};

/** The git tag a version is released under. */
export const version_tag = (version: string): string => `v${version}`;

/** The version a release tag names: `v0.3.0` → `0.3.0` (a bare `0.3.0` is accepted too, so a
 * caller can pass a tag name as-is). Null when the text is not a `v<major>.<minor>.<patch>`. */
export const version_from_tag = (tag: string): string | null => {
	const version = tag.startsWith('v') ? tag.slice(1) : tag;
	return version_parse(version) ? version : null;
};
