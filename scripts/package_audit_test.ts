/**
 * `package_audit.ts`'s crate directories: a manifest cargo reports is read beneath the checkout,
 * and one outside it is refused rather than read.
 */

import { strictEqual } from 'node:assert';
import { test } from 'vitest';

import { to_crate_dir } from './package_audit.ts';

test("a member's manifest names its directory relative to the checkout", () => {
	strictEqual(to_crate_dir('/r/tsv', '/r/tsv/crates/tsv_lang/Cargo.toml'), 'crates/tsv_lang');
	strictEqual(to_crate_dir('/r/tsv', '/r/tsv/Cargo.toml'), '.');
});

test('a manifest outside the checkout has no crate directory', () => {
	strictEqual(to_crate_dir('/r/tsv', '/r/other/Cargo.toml'), null);
	strictEqual(to_crate_dir('/r/tsv', '/r/tsv-fork/crates/x/Cargo.toml'), null);
	strictEqual(to_crate_dir('/r/tsv', '/home/u/.cargo/registry/src/x-1.0.0/Cargo.toml'), null);
	strictEqual(to_crate_dir('/r/tsv', '/r/Cargo.toml'), null);
});
