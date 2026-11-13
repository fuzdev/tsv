/**
 * The package-contents audit: what each crate meant for publishing puts in its crates.io
 * tarball, and what its manifest states to a consumer.
 *
 * ## What it proves
 *
 * For each crate meant for publishing, from `cargo package --list` (the exact file list
 * `cargo publish` would upload, read without building anything):
 *
 * - the tarball carries a `README.md` and a `LICENSE`, and `src/lib.rs` (a listing that
 *   names no source grades nothing, so it fails);
 * - it carries every `NOTICE` the crate directory holds, and the crates listed in
 *   `REQUIRED_NOTICE` (the third-party data they ship is attributed there) must hold one;
 * - it carries no other Markdown — no `CLAUDE.md`, no `AGENTS.md`, no stray doc — so agent
 *   notes and repo docs stay in the repository;
 * - it carries none of the files `MUST_NOT_SHIP` names for it (`tsv_html`'s `entities.json`,
 *   CC BY 4.0 as data, stays in the repository; the crate ships only the table generated
 *   from it);
 * - the crate's `LICENSE` holds the root `LICENSE`'s bytes (read through the symlink each
 *   crate commits, which cargo packages as a regular file);
 * - every Markdown link in its `README.md` is absolute, since crates.io renders the README
 *   away from the repository, where a relative link resolves to nothing.
 *
 * And from one `cargo metadata`, per crate: the `rust-version` is the toolchain
 * `rust-toolchain.toml` pins, as major.minor (the MSRV a consumer reads is the one CI
 * tests), the license is the SPDX `0BSD` — or, for a crate `LICENSE_EXCEPTIONS` names, the
 * expression it states there (`tsv_html` ships WHATWG's table under BSD-3-Clause beside
 * tsv's 0BSD) — and the readme cargo resolved is `README.md`.
 * A crate that ships a binary (`tsv_cli`) must carry a `[package.metadata.binstall]` table
 * whose URLs name exactly the GitHub Release assets the release workflow uploads
 * (`.github/workflows/release.yml`'s build matrix, each triple's asset spelled by
 * `release_asset_name`), every shipped triple but the dynamically-linked musl one mapped from
 * its Rust target (`scripts/release/binstall.ts`); a crate with no binary carries none.
 * The same read cross-checks the publishable set: the members cargo would publish
 * (`publish` unset) must be exactly the crates `docsrs_crates.ts` lists, so the regex
 * reader the other gates share cannot drift from cargo's own reading.
 *
 * ## Which crates
 *
 * The members with a `[package.metadata.docs.rs]` table (`docsrs_crates.ts`, the
 * definition `deps:audit` and `docs:audit:docsrs` share).
 *
 * ## Blind spots
 *
 * It reads the listing, not a built `.crate`: a symlinked `LICENSE` that resolves
 * differently on another platform is caught only by the content compare on the machine
 * that runs it, and it never compiles the packaged sources (`cargo publish --workspace
 * --dry-run --locked` does: `npm run publish:crates`, in CI). It grades that a README
 * exists and links absolutely, never what it says.
 * `--allow-dirty` lists the working tree as it stands, untracked files included, so a run
 * on a dirty tree grades what a publish from that tree would upload.
 *
 * ## Reach
 *
 * The checkout read-only (the manifests, each crate's files, the toolchain pin, the release
 * workflow) and `cargo`, which runs `cargo metadata` and `cargo package --list` in it
 * (`PackageAuditDeps`, built by `scripts/roots/package_audit.ts`).
 */

import { dirname, isAbsolute, join, relative, sep } from 'node:path';

import type { FsReadDir } from '@fuzdev/gro/fs.ts';
import type { SpawnProgram } from '@fuzdev/gro/spawn.ts';

import { docs_rs_crates } from './docsrs_crates.ts';
import {
	binstall_findings,
	type BinstallMetadata,
	workflow_release_triples
} from './release/binstall.ts';

const log = (...args: unknown[]) => console.error(...args);

/** Crates whose tarball must carry a `NOTICE`, for the third-party data they ship. */
const REQUIRED_NOTICE = ['tsv_html'];

/** The SPDX license every publishable crate states, unless `LICENSE_EXCEPTIONS` names it. */
const LICENSE = '0BSD';

/** The crates whose SPDX expression adds third-party code's license to tsv's own. */
const LICENSE_EXCEPTIONS: Record<string, string> = {
	// WHATWG's named-reference table, incorporated into source code (`src/entities_table.rs`)
	tsv_html: '0BSD AND BSD-3-Clause'
};

/** Repository files a crate's tarball must not carry. */
const MUST_NOT_SHIP: Record<string, Array<string>> = {
	tsv_html: ['src/entities.json']
};

interface MetadataPackage {
	name: string;
	manifest_path: string;
	publish: string[] | null;
	rust_version: string | null;
	license: string | null;
	readme: string | null;
	repository: string | null;
	targets: Array<{ kind: Array<string> }>;
	metadata: { binstall?: BinstallMetadata } | null;
}

/** The workflow whose GitHub Release carries the binaries `cargo binstall` fetches. */
const RELEASE_WORKFLOW = '.github/workflows/release.yml';

/** What `run_package_audit` reaches. */
export interface PackageAuditDeps {
	/** The checkout, read for the files graded and cargo's working directory. */
	checkout: FsReadDir;
	/** `cargo`, its output captured. */
	cargo: SpawnProgram;
}

async function cargo(
	deps: PackageAuditDeps,
	args: string[]
): Promise<{ status: number; stdout: string }> {
	const result = await deps.cargo.out(args, { cwd: deps.checkout });
	const status = result.kind === 'exited' && !result.truncated ? result.code : 1;
	if (status !== 0) log(result.stderr.trimEnd());
	return { status, stdout: result.stdout };
}

/** The toolchain channel `rust-toolchain.toml` pins, as major.minor. */
function toolchain_minor(checkout: FsReadDir): string {
	const toml = checkout.read_text_sync('rust-toolchain.toml');
	const channel = /^channel\s*=\s*"(\d+)\.(\d+)(?:\.\d+)?"/m.exec(toml);
	if (!channel) throw new Error('no numeric `channel` in rust-toolchain.toml');
	return `${channel[1]}.${channel[2]}`;
}

/**
 * The directory of `manifest_path` (cargo's absolute path) relative to the checkout, or `null`
 * when it is not beneath it.
 */
export function to_crate_dir(checkout_path: string, manifest_path: string): string | null {
	const rel = relative(checkout_path, dirname(manifest_path));
	if (isAbsolute(rel) || rel === '..' || rel.startsWith(`..${sep}`)) return null;
	return rel === '' ? '.' : rel.split(sep).join('/');
}

/** Whether two byte arrays hold the same bytes. */
const same_bytes = (a: Uint8Array, b: Uint8Array): boolean =>
	a.length === b.length && a.every((byte, i) => byte === b[i]);

/** The link targets in `markdown` (inline, reference definitions, HTML `href`/`src`) that are
 * not absolute URLs. Code (fenced blocks, inline spans) is dropped first: a `let src = "…"` or
 * an `<a href>` shown as code is no link. */
function relative_links(markdown: string): string[] {
	const prose = markdown
		.replace(/^ {0,3}(`{3,}|~{3,})[^\n]*\n[\s\S]*?^ {0,3}\1[`~]*[ \t]*$/gm, '')
		.replace(/(`+)[^`]*?\1/g, '');
	const targets = [
		...[...prose.matchAll(/\]\(([^)\s]+)[^)]*\)/g)].map((m) => m[1]),
		...[...prose.matchAll(/^ {0,3}\[(?!\^)[^\]]+\]:\s*<?([^\s>]+)/gm)].map((m) => m[1]),
		...[...prose.matchAll(/<[a-z][^>]*?\s(?:href|src)\s*=\s*["']([^"']+)["']/gi)].map((m) => m[1])
	];
	return targets.filter((target) => !/^[a-z][a-z0-9+.-]*:/i.test(target));
}

/** Every finding for one crate's tarball listing and files, empty when it passes. */
async function grade_listing(
	checkout: FsReadDir,
	name: string,
	dir: string,
	listing: string[]
): Promise<string[]> {
	const findings: string[] = [];
	const files = new Set(listing);
	for (const required of ['README.md', 'LICENSE', 'src/lib.rs']) {
		if (!files.has(required)) findings.push(`tarball lacks ${required}`);
	}
	const notice_on_disk = await checkout.exists(join(dir, 'NOTICE'));
	if (REQUIRED_NOTICE.includes(name) && !notice_on_disk) {
		findings.push('crate holds no NOTICE, which its shipped data requires');
	}
	if (notice_on_disk && !files.has('NOTICE')) findings.push('tarball lacks the NOTICE');
	for (const file of MUST_NOT_SHIP[name] ?? []) {
		if (!(await checkout.exists(join(dir, file)))) {
			findings.push(`${file} is absent from the crate directory, so its exclusion grades nothing`);
		}
		if (files.has(file)) findings.push(`tarball ships ${file}, which stays in the repository`);
	}
	for (const file of listing) {
		if (/\.md$/i.test(file) && file !== 'README.md') {
			findings.push(`tarball ships ${file} (only README.md belongs in it)`);
		}
	}
	if (await checkout.exists(join(dir, 'LICENSE'))) {
		if (!same_bytes(await checkout.read(join(dir, 'LICENSE')), await checkout.read('LICENSE'))) {
			findings.push("LICENSE differs from the root LICENSE's bytes");
		}
	}
	if (await checkout.exists(join(dir, 'README.md'))) {
		for (const target of relative_links(await checkout.read_text(join(dir, 'README.md')))) {
			findings.push(`README.md links relatively (${target}), which crates.io cannot resolve`);
		}
	}
	return findings;
}

/** Every finding for one crate's resolved manifest, empty when it passes. */
function grade_metadata(pkg: MetadataPackage, msrv: string): string[] {
	const findings: string[] = [];
	if (pkg.rust_version !== msrv) {
		findings.push(
			`rust-version is ${pkg.rust_version ?? 'unset'}, not ${msrv} (rust-toolchain.toml's channel)`
		);
	}
	const license = LICENSE_EXCEPTIONS[pkg.name] ?? LICENSE;
	if (pkg.license !== license) {
		findings.push(`license is ${pkg.license ?? 'unset'}, not ${license}`);
	}
	if (pkg.readme !== 'README.md') {
		findings.push(`readme resolves to ${pkg.readme ?? 'nothing'}, not README.md`);
	}
	return findings;
}

/**
 * The audit (module doc).
 * @returns the exit code: `1` on any finding, else `0`
 */
export async function run_package_audit(deps: PackageAuditDeps): Promise<number> {
	const { checkout } = deps;
	const crates = docs_rs_crates((path) => checkout.read_text_sync(path));
	if (crates.length === 0) {
		log('Error: no workspace crate carries a [package.metadata.docs.rs] table — nothing graded.');
		return 1;
	}
	const msrv = toolchain_minor(checkout);

	const metadata = await cargo(deps, ['metadata', '--no-deps', '--format-version', '1']);
	if (metadata.status !== 0) {
		log('package:audit FAILED — cargo metadata failed');
		return 1;
	}
	const packages: MetadataPackage[] = JSON.parse(metadata.stdout).packages;
	const by_name = new Map(packages.map((p) => [p.name, p]));
	const release_triples = workflow_release_triples(await checkout.read_text(RELEASE_WORKFLOW));

	let failed = 0;

	// cargo's own reading of the publishable set must be the shared reader's
	const listed = new Set(crates.map((c) => c.name));
	const publishable = new Set(packages.filter((p) => p.publish === null).map((p) => p.name));
	const drift = [
		...[...publishable]
			.filter((n) => !listed.has(n))
			.map((n) => `${n} is publishable to cargo but not listed`),
		...[...listed]
			.filter((n) => !publishable.has(n))
			.map((n) => `${n} is listed but cargo would not publish it`)
	];
	for (const finding of drift) log(`✗ publishable set: ${finding}`);
	if (drift.length > 0) failed++;

	for (const c of crates) {
		const pkg = by_name.get(c.name);
		if (!pkg) {
			failed++;
			log(`✗ ${c.name}: not in cargo metadata`);
			continue;
		}
		const dir = to_crate_dir(checkout.path, pkg.manifest_path);
		if (dir === null) {
			failed++;
			log(`✗ ${c.name}: its manifest ${pkg.manifest_path} is outside the checkout`);
			continue;
		}
		const list = await cargo(deps, [
			'package',
			'--list',
			'-p',
			c.name,
			'--locked',
			'--allow-dirty'
		]);
		if (list.status !== 0) {
			failed++;
			log(`✗ ${c.name}: cargo package --list failed`);
			continue;
		}
		const listing = list.stdout.split('\n').filter((line) => line !== '');
		const findings = [
			...(await grade_listing(checkout, c.name, dir, listing)),
			...grade_metadata(pkg, msrv)
		];
		// a crate shipping a binary must let `cargo binstall` find the Release's prebuilt one
		const ships_bin = pkg.targets.some((t) => t.kind.includes('bin'));
		if (ships_bin) {
			for (const finding of binstall_findings(
				pkg.metadata?.binstall,
				pkg.repository,
				release_triples
			)) {
				findings.push(`binstall: ${finding}`);
			}
		} else if (pkg.metadata?.binstall !== undefined) {
			findings.push('binstall: a [package.metadata.binstall] table on a crate with no binary');
		}
		if (findings.length > 0) {
			failed++;
			for (const finding of findings) log(`✗ ${c.name}: ${finding}`);
		} else {
			const binstall = pkg.metadata?.binstall
				? `, binstall over ${release_triples.length} release triple(s)`
				: '';
			log(
				`✓ ${c.name}: ${listing.length} file(s), README + LICENSE, rust-version ${msrv}${binstall}`
			);
		}
	}
	if (failed > 0) {
		log(`package:audit FAILED — ${failed} finding group(s) over ${crates.length} crate(s)`);
		return 1;
	}
	log(`package:audit OK — ${crates.length} crate(s) package as stated`);
	return 0;
}
