/**
 * The workspace crates meant for docs.rs, and so for crates.io: each member whose
 * `Cargo.toml` carries a `[package.metadata.docs.rs]` table, with the feature set that
 * table names. The one definition of "a crate meant for publishing" the gates share
 * (`docs:audit:docsrs` builds each one's docs, `deps:audit` grades each one's
 * dependency graph).
 *
 * The definition is closed: every member, the root package included, either carries the
 * table or says `publish = false` in its `[package]` table, never both and never neither, and
 * reading the list throws otherwise. So a new crate cannot reach crates.io ungraded by
 * either gate — leaving out both is a failure of both, not a silent skip — and
 * `cargo publish --workspace`, which skips exactly the `publish = false` members, publishes
 * the crates these gates graded.
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

/** The workspace's member directories: the root package (`.`, when the root `Cargo.toml`
 * declares one) and the root `Cargo.toml`'s `members` list. */
function workspace_members(): string[] {
	const root = readFileSync('Cargo.toml', 'utf8');
	const members = /^members\s*=\s*\[([^\]]*)\]/m.exec(root);
	if (!members) throw new Error('no `members` list in the root Cargo.toml');
	return [
		...(/^\[package\]/m.test(root) ? ['.'] : []),
		...[...members[1].matchAll(/"([^"]+)"/g)].map((m) => m[1])
	];
}

/** A member's docs.rs settings, or `null` when it is `publish = false`. Throws for a member
 * that is both or neither. */
function docs_rs_crate(member: string): DocsRsCrate | null {
	const path = `${member}/Cargo.toml`;
	const manifest = readFileSync(path, 'utf8');
	const name = /^\[package\][^[]*?^name\s*=\s*"([^"]+)"/ms.exec(manifest)?.[1];
	if (!name) throw new Error(`no package name in ${path}`);
	const unpublished = /^\[package\][^[]*?^publish\s*=\s*false\s*$/ms.test(manifest);
	const table = /^\[package\.metadata\.docs\.rs\]\n((?:(?!\[).*\n?)*)/m.exec(manifest)?.[1];
	if (unpublished && table !== undefined) {
		throw new Error(
			`${name} (${path}) is \`publish = false\` yet carries a [package.metadata.docs.rs] table — drop one`
		);
	}
	if (!unpublished && table === undefined) {
		throw new Error(
			`${name} (${path}) is publishable but carries no [package.metadata.docs.rs] table, so ` +
				'deps:audit and docs:audit:docsrs would not grade it — add the table, or `publish = false`'
		);
	}
	if (table === undefined) return null;
	const features = /^features\s*=\s*\[([^\]]*)\]/m.exec(table)?.[1];
	return {
		name,
		features: features ? [...features.matchAll(/"([^"]+)"/g)].map((m) => m[1]) : [],
		all_features: /^all-features\s*=\s*true/m.test(table),
		no_default_features: /^no-default-features\s*=\s*true/m.test(table)
	};
}

/** Every workspace member carrying a `[package.metadata.docs.rs]` table — every publishable
 * member — in `members` order. Throws when a member is neither tabled nor `publish = false`. */
export function docs_rs_crates(): DocsRsCrate[] {
	return workspace_members()
		.map(docs_rs_crate)
		.filter((c) => c !== null);
}
