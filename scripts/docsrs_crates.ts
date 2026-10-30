/**
 * The workspace crates meant for docs.rs, and so for crates.io: each member whose
 * `Cargo.toml` carries a `[package.metadata.docs.rs]` table, with the feature set that
 * table names. The one definition of "a crate meant for publishing" the gates share
 * (`docs:audit:docsrs` builds each one's docs, `deps:audit` grades each one's
 * dependency graph).
 *
 * A member's `[package.metadata.docs.rs]` table must be written as a table header; an
 * inline spelling is not parsed, so its crate is not listed.
 */

import { readFileSync } from 'node:fs';

/** A crate meant for docs.rs, and the docs.rs settings its manifest names. */
export interface DocsRsCrate {
	name: string;
	features: string[];
	all_features: boolean;
	no_default_features: boolean;
}

/** The workspace's member directories, from the root `Cargo.toml`'s `members` list. */
function workspace_members(): string[] {
	const root = readFileSync('Cargo.toml', 'utf8');
	const members = /^members\s*=\s*\[([^\]]*)\]/m.exec(root);
	if (!members) throw new Error('no `members` list in the root Cargo.toml');
	return [...members[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

/** A member's docs.rs settings, or `null` when it carries no `[package.metadata.docs.rs]`. */
function docs_rs_crate(member: string): DocsRsCrate | null {
	const manifest = readFileSync(`${member}/Cargo.toml`, 'utf8');
	const name = /^\[package\][^[]*?^name\s*=\s*"([^"]+)"/ms.exec(manifest)?.[1];
	if (!name) throw new Error(`no package name in ${member}/Cargo.toml`);
	const table = /^\[package\.metadata\.docs\.rs\]\n((?:(?!\[).*\n?)*)/m.exec(manifest)?.[1];
	if (table === undefined) return null;
	const features = /^features\s*=\s*\[([^\]]*)\]/m.exec(table)?.[1];
	return {
		name,
		features: features ? [...features.matchAll(/"([^"]+)"/g)].map((m) => m[1]) : [],
		all_features: /^all-features\s*=\s*true/m.test(table),
		no_default_features: /^no-default-features\s*=\s*true/m.test(table)
	};
}

/** Every workspace member carrying a `[package.metadata.docs.rs]` table, in `members` order. */
export function docs_rs_crates(): DocsRsCrate[] {
	return workspace_members()
		.map(docs_rs_crate)
		.filter((c) => c !== null);
}
