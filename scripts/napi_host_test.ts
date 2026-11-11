/**
 * The host triple from what a Node process reports, against the spelling Deno's build target
 * gives the same host.
 */

import { strictEqual } from 'node:assert';
import { test } from 'vitest';

import { host_triple, node_host_triple } from './napi_host.ts';

test("a Node host's triple is the one Deno's build target spells for it", () => {
	const hosts: Array<[Parameters<typeof node_host_triple>[0], Parameters<typeof host_triple>[0]]> =
		[
			[
				{ platform: 'linux', arch: 'x64', glibc: true },
				{ os: 'linux', arch: 'x86_64', target: 'x86_64-unknown-linux-gnu' }
			],
			[
				{ platform: 'linux', arch: 'arm64', glibc: true },
				{ os: 'linux', arch: 'aarch64', target: 'aarch64-unknown-linux-gnu' }
			],
			[
				{ platform: 'linux', arch: 'x64', glibc: false },
				{ os: 'linux', arch: 'x86_64', target: 'x86_64-unknown-linux-musl' }
			],
			[
				{ platform: 'darwin', arch: 'arm64', glibc: false },
				{ os: 'darwin', arch: 'aarch64', target: 'aarch64-apple-darwin' }
			],
			[
				{ platform: 'darwin', arch: 'x64', glibc: false },
				{ os: 'darwin', arch: 'x86_64', target: 'x86_64-apple-darwin' }
			],
			[
				{ platform: 'win32', arch: 'x64', glibc: false },
				{ os: 'windows', arch: 'x86_64', target: 'x86_64-pc-windows-msvc' }
			]
		];
	for (const [node, deno] of hosts) {
		strictEqual(node_host_triple(node), host_triple(deno));
	}
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
