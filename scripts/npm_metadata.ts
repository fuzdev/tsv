/**
 * The npm registry-identity metadata shared by EVERY published tsv package —
 * the three wasm packages (`scripts/patch_npm_package.ts`) and the N-API set's
 * loader + platform packages (`scripts/build_napi_packages.ts`). One source so
 * the packages present identically on the registry; before this module the two
 * scripts hand-synced these fields with nothing gating the agreement.
 *
 * `engines` is the one load-bearing field: it must move in lockstep across all
 * packages (a split floor is a support-contract fork, not a cosmetic drift).
 * The floor is 22 — Node 20 is EOL (2026-04) and SvelteKit 3 requires 22+, so
 * tsv's ecosystem consumers already sit there. Note 22.0 < 22.12: unflagged
 * `require(esm)` is NOT guaranteed in-range, so dynamic `import()` stays the
 * universal CommonJS-host path (`scripts/test_napi_npm.ts` gates it).
 *
 * Per-package fields (`description`, `keywords`, `files`, `exports`, `bin`,
 * `sideEffects`) stay in each staging script — they genuinely differ.
 */
export const NPM_SHARED_METADATA = {
	license: 'MIT',
	homepage: 'https://github.com/fuzdev/tsv',
	author: {
		name: 'Ryan Atkinson',
		email: 'mail@ryanatkn.com',
		url: 'https://www.ryanatkn.com/'
	},
	repository: {
		type: 'git',
		url: 'git+https://github.com/fuzdev/tsv.git'
	},
	bugs: 'https://github.com/fuzdev/tsv/issues',
	funding: 'https://www.ryanatkn.com/funding',
	engines: { node: '>=22' }
} as const;
