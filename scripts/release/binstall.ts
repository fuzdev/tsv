/**
 * `cargo binstall`'s view of a release: the `[package.metadata.binstall]` table a crate with a
 * binary carries, graded against the GitHub Release assets the release workflow uploads. Pure:
 * `scripts/package_audit.ts` reads the metadata (`cargo metadata`) and the workflow's text, and
 * this decides (`binstall_test.ts`).
 *
 * The assets are named by npm triple (`release_asset_name`), while binstall resolves a Rust
 * target triple, so each shipped target needs an override naming its asset; a target with no
 * override falls to binstall's `{ target }`-templated default URL, which names no asset, and
 * so to a source build.
 *
 * @module
 */

import { release_asset_name } from '../napi_host.ts';
import { version_tag } from './version.ts';

/** The URL every override's asset hangs off: the tag's GitHub Release downloads. binstall
 * fills `{ repo }` from the crate's `repository` and `{ version }` from the crate version; the
 * tag is `version_tag`'s spelling. */
export const BINSTALL_URL_PREFIX = `{ repo }/releases/download/${version_tag('{ version }')}/`;

/** Why binstall must NOT map a release triple, or null when it must: a musl binary links
 * musl dynamically, so it runs only where musl is the system libc — and binstall also tries a
 * musl target as a glibc host's fallback, where it would install a binary that cannot start. */
export const binstall_unmapped_reason = (triple: string): string | null =>
	triple.endsWith('-musl')
		? "dynamically linked against musl, so it runs only on a musl-libc host — and binstall tries a musl target as a glibc host's fallback, where it could not start"
		: null;

/** The npm-style triple of a Rust target triple (`x86_64-unknown-linux-gnu` →
 * `linux-x64-gnu`), the spelling the release assets carry; null for a target with no such
 * spelling. */
export const npm_triple_of_rust_target = (target: string): string | null => {
	const match = /^(x86_64|aarch64)-(unknown-linux-(gnu|musl)|apple-darwin|pc-windows-msvc)$/.exec(
		target
	);
	if (!match) return null;
	const cpu = match[1] === 'x86_64' ? 'x64' : 'arm64';
	if (match[3]) return `linux-${cpu}-${match[3]}`;
	return match[2] === 'apple-darwin' ? `darwin-${cpu}` : `win32-${cpu}`;
};

/** The triples the release workflow's build matrix ships (`- triple: <t>` rows). */
export const workflow_release_triples = (workflow: string): Array<string> =>
	[...workflow.matchAll(/^\s*- triple: ([\w-]+)\s*$/gm)].map((m) => m[1]!);

/** A crate's `[package.metadata.binstall]` table as `cargo metadata` reports it. */
export interface BinstallMetadata {
	'pkg-url'?: unknown;
	'pkg-fmt'?: unknown;
	'disabled-strategies'?: unknown;
	overrides?: unknown;
}

/**
 * Every finding for one crate's binstall table, empty when it passes: the format is a bare
 * binary; the third-party quick-install source is disabled; every shipped triple but the
 * unmapped ones has exactly one override, naming that triple's asset under the tag's Release;
 * no override names a target the release does not ship; and `{ repo }` resolves to a GitHub
 * repository.
 */
export const binstall_findings = (
	binstall: BinstallMetadata | undefined,
	repository: string | null,
	release_triples: ReadonlyArray<string>
): Array<string> => {
	if (binstall === undefined) return ['no [package.metadata.binstall] table'];
	const findings: Array<string> = [];
	if (release_triples.length === 0) {
		findings.push('the release workflow names no `- triple:` rows to grade against');
	}
	if (repository === null || !/^https:\/\/github\.com\/[\w.-]+\/[\w.-]+$/.test(repository)) {
		findings.push(
			`repository is ${repository ?? 'unset'}, so binstall's { repo } names no GitHub Release`
		);
	}
	if (binstall['pkg-fmt'] !== 'bin') {
		findings.push(
			`pkg-fmt is ${JSON.stringify(binstall['pkg-fmt'])}, not "bin" (the assets are bare binaries)`
		);
	}
	const base = binstall['pkg-url'];
	if (typeof base !== 'string' || !base.startsWith(BINSTALL_URL_PREFIX)) {
		findings.push(`pkg-url ${JSON.stringify(base)} does not start with ${BINSTALL_URL_PREFIX}`);
	}
	const disabled = binstall['disabled-strategies'];
	if (!Array.isArray(disabled) || !disabled.includes('quick-install')) {
		findings.push(
			'disabled-strategies does not name "quick-install" (an unmapped target must build from source, not fetch a third-party build)'
		);
	}
	const overrides =
		typeof binstall.overrides === 'object' && binstall.overrides !== null
			? (binstall.overrides as Record<string, Record<string, unknown>>)
			: {};
	const mapped = new Map<string, string>();
	for (const [target, override] of Object.entries(overrides)) {
		const triple = npm_triple_of_rust_target(target);
		if (triple === null || !release_triples.includes(triple)) {
			findings.push(`override ${target} names a target the release workflow ships no binary for`);
			continue;
		}
		const unmapped = binstall_unmapped_reason(triple);
		if (unmapped !== null) {
			findings.push(`override ${target} maps ${triple}, which must stay unmapped: ${unmapped}`);
			continue;
		}
		if (mapped.has(triple)) {
			findings.push(`overrides ${mapped.get(triple)} and ${target} both map ${triple}`);
		}
		mapped.set(triple, target);
		const expected = BINSTALL_URL_PREFIX + release_asset_name(triple);
		if (override['pkg-url'] !== expected) {
			findings.push(
				`override ${target} pkg-url is ${JSON.stringify(override['pkg-url'])}, not ${expected} (its Release asset)`
			);
		}
		if (override['pkg-fmt'] !== undefined && override['pkg-fmt'] !== 'bin') {
			findings.push(
				`override ${target} pkg-fmt is ${JSON.stringify(override['pkg-fmt'])}, not "bin"`
			);
		}
	}
	for (const triple of release_triples) {
		if (binstall_unmapped_reason(triple) === null && !mapped.has(triple)) {
			findings.push(
				`no override maps ${triple}, so binstall cannot find its asset ${release_asset_name(triple)}`
			);
		}
	}
	return findings;
};
