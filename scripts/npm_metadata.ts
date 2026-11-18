/**
 * What every published tsv package shares on the registry: its identity metadata and its
 * third-party notice. Also home to the reader of `npm pack --json`'s output, whose shape differs
 * across npm majors, so the release-asset collector (`scripts/release_assets.ts`) and the N-API
 * package test (`scripts/test_napi_npm.ts`) read it one way.
 *
 * @module
 */

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
 * `require` of an ES module is NOT guaranteed in-range, so dynamic `import()` stays the
 * universal CommonJS-host path (`scripts/test_napi_npm.ts` gates it).
 *
 * `license` is an SPDX expression: tsv's own code is 0BSD, and every package compiles in
 * the WHATWG named-reference table, which is BSD-3-Clause as source code (`THIRD_PARTY_NOTICE`).
 *
 * Per-package fields (`description`, `keywords`, `files`, `exports`, `bin`,
 * `sideEffects`) stay in each staging script — they genuinely differ.
 */
export const NPM_SHARED_METADATA = {
	license: '0BSD AND BSD-3-Clause',
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
	engines: { node: '>=22' },
	// structural, so a hand `npm publish` of a scoped package can never land
	// restricted — the publish scripts pass `--access public` as well
	publishConfig: { access: 'public' }
} as const;

/**
 * The third-party notice every published tsv package ships, and every GitHub Release
 * carries beside its CLI binaries: the WHATWG license for the named character references
 * table compiled into each engine (`tsv_html`'s named-reference table). BSD 3-Clause, which
 * governs the table as source code, requires its notice in the documentation of any
 * binary distribution, and every wasm bundle, N-API addon and CLI binary is one.
 *
 * One source, the `tsv_html` crate's own `NOTICE`, because that copy must live inside the
 * crate (`cargo package` reads nothing outside it); every package and the Release copy it
 * verbatim, so no second spelling can drift. tsv's own code stays 0BSD (`LICENSE`).
 */
export const THIRD_PARTY_NOTICE = {
	source: 'crates/tsv_html/NOTICE',
	published: 'NOTICE'
} as const;

/** One package's report in `npm pack --json`'s output, as far as the scripts read it. Every
 * field is optional: the output is npm's, not ours, and a reader checks what it uses. */
export interface NpmPackReport {
	filename?: string;
	files?: Array<{ path: string; size: number; mode: number }>;
}

/**
 * The per-package reports in `npm pack --json`'s stdout, in npm's order. npm 11 and older print
 * an array of reports; npm 12 prints an object keyed by package name, dry run or not. Both
 * shapes are read because the release workflow's pinned Node ships the older npm while a local
 * checkout may run the newer one.
 *
 * @throws `Error` - when the output is no JSON, or JSON of neither shape
 */
export const npm_pack_reports = (stdout: string): Array<NpmPackReport> => {
	let parsed: unknown;
	try {
		parsed = JSON.parse(stdout);
	} catch (error) {
		throw new Error(
			`npm pack --json printed no JSON: ${error instanceof Error ? error.message : String(error)}`
		);
	}
	if (Array.isArray(parsed)) return parsed as Array<NpmPackReport>;
	if (typeof parsed === 'object' && parsed !== null) {
		return Object.values(parsed) as Array<NpmPackReport>;
	}
	throw new Error(`npm pack --json printed neither an array nor an object: ${stdout}`);
};
