import { deepStrictEqual } from 'node:assert';

import {
	cargo_workspace_version,
	cargo_workspace_version_set,
	is_bump_level,
	version_bump,
	version_from_tag,
	version_parse,
	version_tag
} from './version.ts';

const cargo = `[workspace]
resolver = "2"

[workspace.package]
version = "0.1.1"
edition = "2024"
rust-version = "1.89"

[workspace.dependencies]
serde = { version = "1.0.0" }
`;

Deno.test('cargo_workspace_version reads the workspace.package table only', () => {
	deepStrictEqual(cargo_workspace_version(cargo), '0.1.1');
	deepStrictEqual(
		cargo_workspace_version(
			'[workspace.package]\nrust-version = "1.89"\n\n[x]\nversion = "9.9.9"\n'
		),
		null
	);
	deepStrictEqual(cargo_workspace_version('[package]\nversion = "0.1.0"\n'), null);
});

Deno.test('cargo_workspace_version_set rewrites that one value', () => {
	const next = cargo_workspace_version_set(cargo, '0.2.0');
	deepStrictEqual(next, cargo.replace('version = "0.1.1"', 'version = "0.2.0"'));
	deepStrictEqual(cargo_workspace_version_set('[package]\nversion = "0.1.0"\n', '0.2.0'), null);
});

Deno.test('version_parse accepts release versions only', () => {
	deepStrictEqual(version_parse('0.1.10'), { major: 0, minor: 1, patch: 10 });
	deepStrictEqual(version_parse('0.2.0-alpha.1'), null);
	deepStrictEqual(version_parse('01.0.0'), null);
	deepStrictEqual(version_parse('1.0'), null);
});

Deno.test('version_bump resets the lower components', () => {
	deepStrictEqual(version_bump('0.1.9', 'patch'), '0.1.10');
	deepStrictEqual(version_bump('0.1.9', 'minor'), '0.2.0');
	deepStrictEqual(version_bump('0.1.9', 'major'), '1.0.0');
	deepStrictEqual(version_bump('0.2.0-alpha.1', 'patch'), null);
});

Deno.test('is_bump_level and version_tag', () => {
	deepStrictEqual(['patch', 'minor', 'major', 'prerelease', undefined].map(is_bump_level), [
		true,
		true,
		true,
		false,
		false
	]);
	deepStrictEqual(version_tag('0.1.1'), 'v0.1.1');
});

Deno.test('version_from_tag accepts v-prefixed and bare semver only', () => {
	deepStrictEqual(version_from_tag('v0.3.0'), '0.3.0');
	deepStrictEqual(version_from_tag('0.3.0'), '0.3.0');
	deepStrictEqual(version_from_tag('v0.3'), null);
	deepStrictEqual(version_from_tag('v0.3.0-rc.1'), null);
	deepStrictEqual(version_from_tag('main'), null);
});
