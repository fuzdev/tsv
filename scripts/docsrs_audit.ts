/**
 * The docs.rs-shaped doc build: every crate meant for docs.rs documented on its own, at the
 * feature set docs.rs will use, with a broken intra-doc link denied.
 *
 * ## What it proves
 *
 * That each crate's docs.rs build succeeds. docs.rs builds one published crate at a time,
 * at that crate's own features (its `[package.metadata.docs.rs]` table, else its defaults),
 * and the copied `[workspace.lints.rustdoc]` denies a broken link there — so a link to an
 * item behind a feature docs.rs does not turn on fails the crate's whole docs.rs page.
 * `docs:audit` cannot see it: it builds the workspace with `--all-features`, which turns
 * every gate on, and a `--workspace` build unifies features across crates, so a link that
 * resolves only through another crate's features resolves there too. Each run here is
 * `cargo doc -p <crate> --no-deps` alone.
 *
 * Links from public docs to private items are WARNED and counted, not denied: docs.rs
 * documents public items only, so each is an inert link on the published page, but this
 * codebase's docs link private items on purpose (`docs:audit` grades them as navigation
 * under `--document-private-items`), and many go once the public surface narrows. The
 * per-crate count is printed so it is visible.
 *
 * ## Which crates, at which features
 *
 * A crate is graded when its `Cargo.toml` carries a `[package.metadata.docs.rs]` table —
 * the table that says the crate is meant for docs.rs, and the one place its feature set
 * is written, read here rather than restated. `features`, `all-features` and
 * `no-default-features` are honored; `targets` is not (this builds the host only). A
 * workspace with no such table grades nothing and fails.
 *
 * ## Build world
 *
 * Its own target directory (`target/docsrs`): the lint flags differ from `docs:audit`'s,
 * and sharing `target/doc` would make the two gates rebuild each other's output on every
 * run.
 */

import { spawnSync } from 'node:child_process';

import { docs_rs_crates } from './docsrs_crates.ts';

const log = (...args: unknown[]) => console.error(...args);

/** The flags docs.rs's build and this one share, plus the gate's lint levels. */
const RUSTDOCFLAGS = [
	'--cfg docsrs',
	'-D rustdoc::broken_intra_doc_links',
	'-D rustdoc::invalid_html_tags',
	'-D rustdoc::bare_urls',
	'-D rustdoc::redundant_explicit_links',
	'-W rustdoc::private_intra_doc_links'
].join(' ');

const TARGET_DIR = 'target/docsrs';

function run(): never {
	const crates = docs_rs_crates();
	if (crates.length === 0) {
		log('Error: no workspace crate carries a [package.metadata.docs.rs] table — nothing graded.');
		process.exit(1);
	}

	let failed = 0;
	for (const c of crates) {
		const args = ['doc', '-p', c.name, '--no-deps', '--quiet'];
		if (c.all_features) args.push('--all-features');
		if (c.no_default_features) args.push('--no-default-features');
		if (c.features.length > 0) args.push('--features', c.features.join(','));
		const result = spawnSync('cargo', args, {
			encoding: 'utf8',
			env: { ...process.env, RUSTDOCFLAGS, CARGO_TARGET_DIR: TARGET_DIR },
			stdio: ['ignore', 'ignore', 'pipe']
		});
		const stderr = result.stderr ?? '';
		const private_links = stderr.match(/links to private item/g)?.length ?? 0;
		const shape = c.features.length > 0 ? ` --features ${c.features.join(',')}` : '';
		if (result.status !== 0) {
			failed++;
			log(stderr.trimEnd());
			log(`✗ ${c.name}${shape}: the docs.rs build fails`);
		} else {
			log(`✓ ${c.name}${shape}: ${private_links} public→private link(s) (warned)`);
		}
	}
	if (failed > 0) {
		log(`docs:audit:docsrs FAILED — ${failed} of ${crates.length} crate(s)`);
		process.exit(1);
	}
	log(`docs:audit:docsrs OK — ${crates.length} crate(s) build their docs.rs docs`);
	process.exit(0);
}

run();
