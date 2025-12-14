// Canonical dependency versions - source of truth for all JS tooling
// Used by: sidecar.ts, benches/deno/lib/canonical.ts

export const VERSIONS = {
	prettier: '3.7.4',
	'prettier-plugin-svelte': '3.4.0',
	svelte: '5.45.8',
	acorn: '8.15.0',
	'@sveltejs/acorn-typescript': '1.0.8',
} as const;
