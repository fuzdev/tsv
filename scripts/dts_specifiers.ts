/**
 * The relative-specifier rule every shipped `.d.ts` follows, as one test both package
 * suites register (`scripts/test_npm.ts` over the wasm packages, `scripts/test_napi_npm.ts`
 * over the native loader).
 *
 * Every relative specifier in a shipped declaration must carry the `.js` extension. Under
 * `moduleResolution: node16`/`nodenext` an extensionless one is TS2834/TS2835 — errors
 * raised from INSIDE the package, at any consumer without `skipLibCheck` — and the same
 * class has shipped in both the generated declarations and the napi loader's hand-written
 * one. A regex is not a typechecker, but it pins this class without a `tsc` dependency in
 * the package suites.
 *
 * @module
 */

import { it } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

/**
 * Register the test over one staged package: every `.d.ts` its `package.json` `files`
 * list ships is read, and any relative specifier without a runtime extension fails it.
 *
 * @param package_dir - the staged package's directory
 */
export function register_dts_specifier_test(package_dir: string): void {
	it('every .d.ts relative specifier carries the .js extension', () => {
		const files: Array<string> = JSON.parse(
			readFileSync(join(package_dir, 'package.json'), 'utf8')
		).files;
		const bad: Array<string> = [];
		for (const rel of files.filter((file) => file.endsWith('.d.ts'))) {
			const source = readFileSync(join(package_dir, rel), 'utf8');
			for (const [, spec] of source.matchAll(/(?:from|import\()\s*['"](\.[^'"]*)['"]/g)) {
				if (!/\.(?:js|mjs|cjs|json)$/.test(spec!)) bad.push(`${rel}: ${spec}`);
			}
		}
		assert.deepEqual(
			bad,
			[],
			`extensionless relative specifiers in shipped .d.ts:\n${bad.join('\n')}`
		);
	});
}
