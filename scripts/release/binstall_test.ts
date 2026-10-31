import { deepStrictEqual, ok } from 'node:assert';

import {
	BINSTALL_URL_PREFIX,
	binstall_findings,
	binstall_unmapped_reason,
	npm_triple_of_rust_target,
	workflow_release_triples
} from './binstall.ts';

const triples = [
	'linux-x64-gnu',
	'linux-arm64-gnu',
	'linux-x64-musl',
	'darwin-arm64',
	'darwin-x64',
	'win32-x64'
];

const url = (asset: string) => `{ repo }/releases/download/v{ version }/${asset}`;

const good = () => ({
	'pkg-url': '{ repo }/releases/download/v{ version }/tsv-{ target }{ binary-ext }',
	'pkg-fmt': 'bin',
	'disabled-strategies': ['quick-install'],
	overrides: {
		'x86_64-unknown-linux-gnu': { 'pkg-url': url('tsv-linux-x64-gnu') },
		'aarch64-unknown-linux-gnu': { 'pkg-url': url('tsv-linux-arm64-gnu') },
		'aarch64-apple-darwin': { 'pkg-url': url('tsv-darwin-arm64') },
		'x86_64-apple-darwin': { 'pkg-url': url('tsv-darwin-x64') },
		'x86_64-pc-windows-msvc': { 'pkg-url': url('tsv-win32-x64.exe') }
	} as Record<string, Record<string, unknown>>
});

const REPO = 'https://github.com/fuzdev/tsv';

Deno.test('the release URL prefix is the tag spelling', () => {
	deepStrictEqual(BINSTALL_URL_PREFIX, '{ repo }/releases/download/v{ version }/');
});

Deno.test('npm_triple_of_rust_target spells the asset triples', () => {
	deepStrictEqual(npm_triple_of_rust_target('x86_64-unknown-linux-gnu'), 'linux-x64-gnu');
	deepStrictEqual(npm_triple_of_rust_target('aarch64-unknown-linux-gnu'), 'linux-arm64-gnu');
	deepStrictEqual(npm_triple_of_rust_target('x86_64-unknown-linux-musl'), 'linux-x64-musl');
	deepStrictEqual(npm_triple_of_rust_target('aarch64-apple-darwin'), 'darwin-arm64');
	deepStrictEqual(npm_triple_of_rust_target('x86_64-pc-windows-msvc'), 'win32-x64');
	deepStrictEqual(npm_triple_of_rust_target('i686-pc-windows-msvc'), null);
	deepStrictEqual(npm_triple_of_rust_target('x86_64-pc-windows-gnu'), null);
});

Deno.test('every musl triple stays unmapped, nothing else does', () => {
	ok(binstall_unmapped_reason('linux-x64-musl') !== null);
	ok(binstall_unmapped_reason('linux-arm64-musl') !== null);
	deepStrictEqual(binstall_unmapped_reason('linux-x64-gnu'), null);
	deepStrictEqual(binstall_unmapped_reason('win32-x64'), null);
});

Deno.test('workflow_release_triples reads the matrix rows', () => {
	const workflow =
		'jobs:\n  build:\n    strategy:\n      matrix:\n        include:\n' +
		'          - triple: linux-x64-gnu\n            runner: ubuntu-24.04\n' +
		'          - triple: win32-x64\n            runner: windows-2025\n' +
		'      # - triple: commented-out\n';
	deepStrictEqual(workflow_release_triples(workflow), ['linux-x64-gnu', 'win32-x64']);
});

Deno.test('the shipped table passes', () => {
	deepStrictEqual(binstall_findings(good(), REPO, triples), []);
});

Deno.test('an asset name that is not the release asset fails', () => {
	const meta = good();
	meta.overrides['x86_64-unknown-linux-gnu'] = { 'pkg-url': url('tsv-linux-x64') };
	const findings = binstall_findings(meta, REPO, triples);
	deepStrictEqual(findings.length, 1);
	ok(findings[0]!.includes('tsv-linux-x64-gnu'), findings[0]);
	// the Windows asset keeps its .exe
	const win = good();
	win.overrides['x86_64-pc-windows-msvc'] = { 'pkg-url': url('tsv-win32-x64') };
	deepStrictEqual(binstall_findings(win, REPO, triples).length, 1);
	// a tag spelled without the v
	const tag = good();
	tag.overrides['aarch64-apple-darwin'] = {
		'pkg-url': '{ repo }/releases/download/{ version }/tsv-darwin-arm64'
	};
	deepStrictEqual(binstall_findings(tag, REPO, triples).length, 1);
});

Deno.test('coverage: every shipped triple but musl is mapped, and nothing else', () => {
	const missing = good();
	delete missing.overrides['x86_64-apple-darwin'];
	const findings = binstall_findings(missing, REPO, triples);
	deepStrictEqual(findings.length, 1);
	ok(findings[0]!.includes('darwin-x64'), findings[0]);

	const musl = good();
	musl.overrides['x86_64-unknown-linux-musl'] = { 'pkg-url': url('tsv-linux-x64-musl') };
	ok(binstall_findings(musl, REPO, triples)[0]!.includes('must stay unmapped'));

	const unshipped = good();
	unshipped.overrides['aarch64-pc-windows-msvc'] = { 'pkg-url': url('tsv-win32-arm64.exe') };
	ok(binstall_findings(unshipped, REPO, triples)[0]!.includes('ships no binary'));

	// a workflow that grows a triple needs its override
	const grown = binstall_findings(good(), REPO, [...triples, 'linux-arm64-musl', 'win32-arm64']);
	deepStrictEqual(grown.length, 1);
	ok(grown[0]!.includes('win32-arm64'), grown[0]);
});

Deno.test('format, strategies, base URL and repository are graded', () => {
	deepStrictEqual(binstall_findings(undefined, REPO, triples), [
		'no [package.metadata.binstall] table'
	]);
	const tgz = { ...good(), 'pkg-fmt': 'tgz' };
	deepStrictEqual(binstall_findings(tgz, REPO, triples).length, 1);
	const quick = { ...good(), 'disabled-strategies': [] };
	deepStrictEqual(binstall_findings(quick, REPO, triples).length, 1);
	const base = { ...good(), 'pkg-url': 'https://example.com/tsv' };
	deepStrictEqual(binstall_findings(base, REPO, triples).length, 1);
	deepStrictEqual(binstall_findings(good(), null, triples).length, 1);
	deepStrictEqual(binstall_findings(good(), 'https://gitlab.com/fuzdev/tsv', triples).length, 1);
	deepStrictEqual(binstall_findings(good(), REPO, []).length, 6);
});
