/**
 * `sidecar_install.ts`: the install is current exactly when npm's hidden lockfile holds the
 * committed lock's packages and each installed package's `package.json` names its locked
 * version.
 */

import { strictEqual, throws } from 'node:assert';
import { test } from 'vitest';

import { sidecar_install_state } from './sidecar_install.ts';

const committed = JSON.stringify({
	name: 'tsv-sidecar',
	packages: {
		'': { dependencies: { esrap: '2.4.0' } },
		'node_modules/esrap': { version: '2.4.0', dependencies: { a: '^1.0.0', b: '^2.0.0' } }
	}
});

const hidden_lock = JSON.stringify({
	packages: {
		'node_modules/esrap': { dependencies: { b: '^2.0.0', a: '^1.0.0' }, version: '2.4.0' }
	}
});

/** A reader over `files`, keyed by their paths beneath the sidecar's directory. */
const reader =
	(files: Record<string, string>) =>
	(path: string): string | null =>
		files[path] ?? null;

/** The files of an install holding `installed_lock` and esrap's `manifest`. */
const install = (installed_lock: string, manifest?: string): Record<string, string> => ({
	'node_modules/.package-lock.json': installed_lock,
	...(manifest === undefined ? {} : { 'node_modules/esrap/package.json': manifest })
});

test('the hidden lockfile holding the same packages, in any key order, is current', () => {
	strictEqual(
		sidecar_install_state(committed, reader(install(hidden_lock, '{"version": "2.4.0"}'))),
		'current'
	);
});

test('no hidden lockfile is missing', () => {
	strictEqual(sidecar_install_state(committed, reader({})), 'missing');
});

test('another version, an extra or a missing package, or an unreadable file is stale', () => {
	const with_packages = (packages: unknown): string => JSON.stringify({ packages });
	for (const installed of [
		with_packages({ 'node_modules/esrap': { version: '2.3.1', dependencies: {} } }),
		with_packages({
			'node_modules/esrap': { version: '2.4.0', dependencies: { a: '^1.0.0', b: '^2.0.0' } },
			'node_modules/extra': { version: '1.0.0' }
		}),
		with_packages({}),
		'{',
		'{}'
	]) {
		strictEqual(
			sidecar_install_state(committed, reader(install(installed, '{"version": "2.4.0"}'))),
			'stale',
			installed
		);
	}
});

test("an installed package.json that isn't its lock entry's version, or is gone, is stale", () => {
	for (const manifest of [undefined, '{"version": "2.4.1"}', '{}', '{']) {
		strictEqual(
			sidecar_install_state(committed, reader(install(hidden_lock, manifest))),
			'stale',
			manifest
		);
	}
});

test('a committed lock with no packages is a broken checkout, not an install to fix', () => {
	throws(() => sidecar_install_state('{}', reader({})), /has no `packages` object/);
});

test('a scoped package is checked like any other', () => {
	const packages = { 'node_modules/@scope/pkg': { version: '1.0.0' } };
	const scoped = JSON.stringify({ packages: { '': {}, ...packages } });
	const files = (manifest: string) => ({
		'node_modules/.package-lock.json': JSON.stringify({ packages }),
		'node_modules/@scope/pkg/package.json': manifest
	});
	strictEqual(sidecar_install_state(scoped, reader(files('{"version": "1.0.0"}'))), 'current');
	strictEqual(sidecar_install_state(scoped, reader(files('{"version": "1.0.1"}'))), 'stale');
});
