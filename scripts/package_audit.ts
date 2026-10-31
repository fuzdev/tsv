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
 * - the crate's `LICENSE` holds the root `LICENSE`'s bytes (read through the symlink each
 *   crate commits, which cargo packages as a regular file);
 * - every Markdown link in its `README.md` is absolute, since crates.io renders the README
 *   away from the repository, where a relative link resolves to nothing.
 *
 * And from one `cargo metadata`, per crate: the `rust-version` is the toolchain
 * `rust-toolchain.toml` pins, as major.minor (the MSRV a consumer reads is the one CI
 * tests), the license is the SPDX `0BSD`, and the readme cargo resolved is `README.md`.
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
 * --dry-run --locked` does: `deno task publish:crates`, in CI). It grades that a README
 * exists and links absolutely, never what it says.
 * `--allow-dirty` lists the working tree as it stands, untracked files included, so a run
 * on a dirty tree grades what a publish from that tree would upload.
 */

import { spawnSync } from 'node:child_process';
import { readFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';

import { docs_rs_crates } from './docsrs_crates.ts';
import {
	binstall_findings,
	type BinstallMetadata,
	workflow_release_triples
} from './release/binstall.ts';

const log = (...args: unknown[]) => console.error(...args);

/** Crates whose tarball must carry a `NOTICE`, for the third-party data they ship. */
const REQUIRED_NOTICE = ['tsv_html'];

/** The SPDX license every publishable crate states. */
const LICENSE = '0BSD';

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

function cargo(args: string[]): { status: number; stdout: string } {
	const result = spawnSync('cargo', args, {
		encoding: 'utf8',
		stdio: ['ignore', 'pipe', 'pipe'],
		maxBuffer: 64 * 1024 * 1024
	});
	if (result.status !== 0) log(result.stderr.trimEnd());
	return { status: result.status ?? 1, stdout: result.stdout ?? '' };
}

/** The toolchain channel `rust-toolchain.toml` pins, as major.minor. */
function toolchain_minor(): string {
	const toml = readFileSync('rust-toolchain.toml', 'utf8');
	const channel = /^channel\s*=\s*"(\d+)\.(\d+)(?:\.\d+)?"/m.exec(toml);
	if (!channel) throw new Error('no numeric `channel` in rust-toolchain.toml');
	return `${channel[1]}.${channel[2]}`;
}

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
function grade_listing(name: string, dir: string, listing: string[]): string[] {
	const findings: string[] = [];
	const files = new Set(listing);
	for (const required of ['README.md', 'LICENSE', 'src/lib.rs']) {
		if (!files.has(required)) findings.push(`tarball lacks ${required}`);
	}
	const notice_on_disk = existsSync(join(dir, 'NOTICE'));
	if (REQUIRED_NOTICE.includes(name) && !notice_on_disk) {
		findings.push('crate holds no NOTICE, which its shipped data requires');
	}
	if (notice_on_disk && !files.has('NOTICE')) findings.push('tarball lacks the NOTICE');
	for (const file of listing) {
		if (/\.md$/i.test(file) && file !== 'README.md') {
			findings.push(`tarball ships ${file} (only README.md belongs in it)`);
		}
	}
	if (existsSync(join(dir, 'LICENSE'))) {
		if (!readFileSync(join(dir, 'LICENSE')).equals(readFileSync('LICENSE'))) {
			findings.push("LICENSE differs from the root LICENSE's bytes");
		}
	}
	if (existsSync(join(dir, 'README.md'))) {
		for (const target of relative_links(readFileSync(join(dir, 'README.md'), 'utf8'))) {
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
	if (pkg.license !== LICENSE)
		findings.push(`license is ${pkg.license ?? 'unset'}, not ${LICENSE}`);
	if (pkg.readme !== 'README.md') {
		findings.push(`readme resolves to ${pkg.readme ?? 'nothing'}, not README.md`);
	}
	return findings;
}

function run(): never {
	const crates = docs_rs_crates();
	if (crates.length === 0) {
		log('Error: no workspace crate carries a [package.metadata.docs.rs] table — nothing graded.');
		process.exit(1);
	}
	const msrv = toolchain_minor();

	const metadata = cargo(['metadata', '--no-deps', '--format-version', '1']);
	if (metadata.status !== 0) {
		log('package:audit FAILED — cargo metadata failed');
		process.exit(1);
	}
	const packages: MetadataPackage[] = JSON.parse(metadata.stdout).packages;
	const by_name = new Map(packages.map((p) => [p.name, p]));
	const release_triples = workflow_release_triples(readFileSync(RELEASE_WORKFLOW, 'utf8'));

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
		const dir = dirname(pkg.manifest_path);
		const list = cargo(['package', '--list', '-p', c.name, '--locked', '--allow-dirty']);
		if (list.status !== 0) {
			failed++;
			log(`✗ ${c.name}: cargo package --list failed`);
			continue;
		}
		const listing = list.stdout.split('\n').filter((line) => line !== '');
		const findings = [...grade_listing(c.name, dir, listing), ...grade_metadata(pkg, msrv)];
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
		process.exit(1);
	}
	log(`package:audit OK — ${crates.length} crate(s) package as stated`);
	process.exit(0);
}

run();
