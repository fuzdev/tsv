/**
 * The host triple from what a Node process reports: each platform package's own spelling.
 */

import { strictEqual } from 'node:assert';
import { test } from 'vitest';

import { node_host_triple } from './napi_host.ts';

test("a Node host's triple is its platform package's name", () => {
	const hosts: Array<[Parameters<typeof node_host_triple>[0], string]> = [
		[{ platform: 'linux', arch: 'x64', glibc: true }, 'linux-x64-gnu'],
		[{ platform: 'linux', arch: 'arm64', glibc: true }, 'linux-arm64-gnu'],
		[{ platform: 'linux', arch: 'x64', glibc: false }, 'linux-x64-musl'],
		[{ platform: 'darwin', arch: 'arm64', glibc: false }, 'darwin-arm64'],
		[{ platform: 'darwin', arch: 'x64', glibc: false }, 'darwin-x64'],
		[{ platform: 'win32', arch: 'x64', glibc: false }, 'win32-x64']
	];
	for (const [node, triple] of hosts) strictEqual(node_host_triple(node), triple);
});

test('a Linux host without a glibc is musl', () => {
	strictEqual(
		node_host_triple({ platform: 'linux', arch: 'arm64', glibc: false }),
		'linux-arm64-musl'
	);
	strictEqual(node_host_triple({ platform: 'linux', arch: 'x64', glibc: true }), 'linux-x64-gnu');
});

test('only Linux reads the libc', () => {
	strictEqual(node_host_triple({ platform: 'win32', arch: 'x64', glibc: true }), 'win32-x64');
	strictEqual(node_host_triple({ platform: 'freebsd', arch: 'x64', glibc: false }), 'freebsd-x64');
});
