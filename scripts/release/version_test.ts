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
	deepStrictEqual(next, {
		ok: true,
		text: cargo.replace('version = "0.1.1"', 'version = "0.2.0"')
	});
	deepStrictEqual(cargo_workspace_version_set('[package]\nversion = "0.1.0"\n', '0.2.0'), {
		ok: false,
		error: 'no [workspace.package] version in Cargo.toml'
	});
});

Deno.test(
	"cargo_workspace_version_set moves the path entries' exact pins in the same write",
	() => {
		const pinned = `[workspace.package]
version = "0.7.1"

[workspace.dependencies]
# a comment naming version = "=0.7.1" is kept
tsv_lang = { path = "crates/tsv_lang", version = "=0.7.1" }
tsv_ts = { path = "crates/tsv_ts", version = "=0.7.1", default-features = false }
tsv_arena = { path = "crates/tsv_arena", default-features = false }
pinned_registry = { version = "=1.2.3" }
serde = "1"

[package]
name = "x"
version = "=0.7.1"
`;
		const next = cargo_workspace_version_set(pinned, '0.8.0');
		const text = pinned
			.replace('version = "0.7.1"', 'version = "0.8.0"')
			.replace('tsv_lang", version = "=0.7.1"', 'tsv_lang", version = "=0.8.0"')
			.replace('tsv_ts", version = "=0.7.1"', 'tsv_ts", version = "=0.8.0"');
		deepStrictEqual(next, { ok: true, text });
		// idempotent: a resumed release repeats the write and changes nothing
		deepStrictEqual(cargo_workspace_version_set(text, '0.8.0'), next);
	}
);

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

Deno.test("cargo_workspace_version_set leaves the repo's Cargo.toml no old version to name", () => {
	// the setter refuses the spellings it can see but not move (a non-exact or single-quoted
	// pin, a `[workspace.dependencies.<name>]` table); this pins the real file against the ones
	// it cannot see (dotted keys), which would keep the old version
	const cargo_toml = Deno.readTextFileSync('Cargo.toml');
	const current = cargo_workspace_version(cargo_toml)!;
	const named = new RegExp(`(?<![\\d.])${current.replaceAll('.', '\\.')}(?![\\d.])`);
	const next = cargo_workspace_version_set(cargo_toml, '999.0.0');
	if (!next.ok) throw Error(next.error);
	deepStrictEqual(named.test(next.text), false);
});

Deno.test('cargo_workspace_version_set refuses a path entry pin it cannot move', () => {
	const head = '[workspace.package]\nversion = "0.7.1"\n\n[workspace.dependencies]\n';
	for (const entry of [
		'tsv_lang = { path = "crates/tsv_lang", version = "0.7.1" }',
		'tsv_lang = { path = "crates/tsv_lang", version = "^0.7.1" }',
		'tsv_lang = { path = "crates/tsv_lang", version = \'=0.7.1\' }',
		'tsv_lang = { version = "0.7", path = "crates/tsv_lang" }',
		'tsv_lang = { path = "crates/tsv_lang", version = "=0.7.1, <1" }',
		'\t"tsv_lang" = { path = "crates/tsv_lang", version = "0.7.1" }'
	]) {
		deepStrictEqual(cargo_workspace_version_set(`${head}${entry}\n`, '0.8.0'), {
			ok: false,
			error: `Cargo.toml has a [workspace.dependencies] path entry whose version is not an exact "=x.y.z" pin the bump can move:\n    ${entry.trim()}`
		});
	}
	// a path entry with no version at all (an unpublished member) and a registry pin are not
	// the bump's to move
	const untouched = `${head}tsv_arena = { path = "crates/tsv_arena" } # version = "1"\nzmij = { version = "1" }\n`;
	deepStrictEqual(cargo_workspace_version_set(untouched, '0.8.0'), {
		ok: true,
		text: untouched.replace('version = "0.7.1"', 'version = "0.8.0"')
	});
});

Deno.test('cargo_workspace_version_set refuses a dependency declared as its own table', () => {
	const cargo_toml =
		'[workspace.package]\nversion = "0.7.1"\n\n[workspace.dependencies.tsv_lang]\npath = "crates/tsv_lang"\nversion = "=0.7.1"\n';
	deepStrictEqual(cargo_workspace_version_set(cargo_toml, '0.8.0'), {
		ok: false,
		error:
			'Cargo.toml declares [workspace.dependencies.tsv_lang] — the version bump rewrites inline [workspace.dependencies] entries only'
	});
	const indented = cargo_toml.replace('[workspace.dependencies.', '  [workspace.dependencies.');
	deepStrictEqual(cargo_workspace_version_set(indented, '0.8.0').ok, false);
});
