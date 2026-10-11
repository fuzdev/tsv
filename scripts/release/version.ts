/**
 * The release version: where it is declared (`Cargo.toml`'s `[workspace.package]` table, which
 * wasm-pack stamps into the npm package) and how a bump moves it. Pure text helpers, no I/O, so
 * the reader and the writer share one grammar.
 *
 * @module
 */

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

/** The `Cargo.toml` text with its `[workspace.package]` version replaced, or null when it
 * declares none. Every other byte is kept. */
export const cargo_workspace_version_set = (cargo_toml: string, version: string): string | null =>
	WORKSPACE_VERSION.test(cargo_toml)
		? cargo_toml.replace(WORKSPACE_VERSION, `$1"${version}"`)
		: null;

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
