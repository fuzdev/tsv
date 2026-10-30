/**
 * The docs.rs-shaped doc build: every crate meant for docs.rs documented on its own, at the
 * feature set docs.rs will use (and at its defaults, where those differ), with a broken
 * intra-doc link denied.
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
 * A link from public docs to a private item is DENIED: docs.rs documents public items only,
 * so each is an inert link on the published page. The lint cannot tell a hidden item from
 * a rendered one, and the hidden internals link private items on purpose (`docs:audit`
 * grades them as navigation under `--document-private-items`), so each hidden module or
 * item whose docs do carries `#[expect(rustdoc::private_intra_doc_links)]`. The lint fires
 * only in this public-only build, so this is also the one build that sees such an
 * expectation go stale — `unfulfilled_lint_expectations` is denied here too.
 *
 * ## Which crates, at which features
 *
 * A crate is graded when its `Cargo.toml` carries a `[package.metadata.docs.rs]` table —
 * the table that says the crate is meant for docs.rs, and the one place its feature set
 * is written, read here rather than restated. `features`, `all-features` and
 * `no-default-features` are honored; `targets` is not (this builds the host only). Every
 * member without the table must be `publish = false` (`docsrs_crates.ts` refuses one that is
 * neither), so the graded set is every publishable member. A workspace with no such table
 * grades nothing and fails.
 *
 * ## The default-features leg
 *
 * A crate whose table names a feature set other than its defaults is graded a second time
 * at its defaults: what a consumer's own `cargo doc` builds after `cargo add`, and the set
 * a link written against the docs.rs features is most likely to break in (a `json` item
 * linked from an always-on page). A crate documented at its defaults on docs.rs is graded
 * once.
 *
 * ## Rendered links
 *
 * A link that resolves can still render dead. A link to a `#[doc(hidden)]` item resolves,
 * so no lint fires, yet the item is not on the page: a hidden member (method, field,
 * variant) renders an `<a>` whose `#anchor` the target page lacks, and a hidden page item
 * (fn, struct, module) renders as inert bracketed code, `[<code>name</code>]`, with no
 * `<a>` at all. Once every build has run, each crate's render is read back: every
 * relative href into a graded crate's pages must name a page that exists and, with a
 * fragment, an `id` that page carries (raw strings — rustdoc percent-encodes ids and hrefs
 * alike), and every inert bracketed code span is counted. Each `cargo doc -p <crate>
 * --no-deps` writes only that crate's directory, and a crate's docs.rs leg runs LAST, so
 * what is on disk by then is every crate's docs.rs render, and a cross-crate href is checked
 * against its sibling's. The crates build in `members` order, which lists each after its
 * dependencies — and must: rustdoc links into a dependency only when that dependency's render
 * directory already exists, so a crate documented ahead of one it links renders the link
 * inert, which fails the gate. A crate whose render holds
 * no `index.html` FAILS: an empty read-back would otherwise report no dead renders.
 * Links into anything else (std, external crates, source pages, rustdoc's own files) are
 * not graded. A hidden link written without backticks renders as plain `[name]` text,
 * which no scan can tell from prose, so it is not counted (a private one is the lint's).
 *
 * A dead render FAILS the gate. `-v` lists every one; a crate with at most `LIST_LIMIT`
 * lists them anyway.
 *
 * ## Stale pages
 *
 * rustdoc never deletes a page it no longer writes, so a page left by an earlier run —
 * an item since hidden, renamed or removed — would make an href into it read as alive.
 * Each crate's render directory is therefore removed before each of its builds, and the
 * read-back sees only what this run wrote: every graded crate is rebuilt every run, and a
 * failed build fails the gate before the read-back.
 *
 * ## Build world
 *
 * Its own target directory (`target/docsrs`): the lint flags differ from `docs:audit`'s,
 * and sharing `target/doc` would make the two gates rebuild each other's output on every
 * run.
 */

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { dirname, join, normalize } from 'node:path';

import { docs_rs_crates } from './docsrs_crates.ts';

const log = (...args: unknown[]) => console.error(...args);

/** The flags docs.rs's build and this one share, plus the gate's lint levels. */
const RUSTDOCFLAGS = [
	'--cfg docsrs',
	'-D rustdoc::broken_intra_doc_links',
	'-D rustdoc::invalid_html_tags',
	'-D rustdoc::bare_urls',
	'-D rustdoc::redundant_explicit_links',
	'-D rustdoc::private_intra_doc_links',
	'-D unfulfilled_lint_expectations'
].join(' ');

const TARGET_DIR = 'target/docsrs';

/** A crate with at most this many dead renders lists them without `-v`. */
const LIST_LIMIT = 20;

const verbose = process.argv.includes('-v') || process.argv.includes('--verbose');

/** A crate's render directory, `target/docsrs/doc/<crate>`. The name is a package name read
 * from a manifest; anything but `[A-Za-z0-9_-]` is refused rather than joined into a path
 * the script then removes. */
function render_dir(crate_name: string): string {
	if (!/^[A-Za-z0-9_-]+$/.test(crate_name)) {
		throw Error(`refusing an unexpected crate name: ${JSON.stringify(crate_name)}`);
	}
	return join(TARGET_DIR, 'doc', crate_name.replaceAll('-', '_'));
}

/** One rendered page's ids, read once per run. */
const page_ids = new Map<string, Set<string> | null>();

function ids_of(page: string): Set<string> | null {
	let ids = page_ids.get(page);
	if (ids === undefined) {
		ids = existsSync(page)
			? new Set([...readFileSync(page, 'utf8').matchAll(/\sid="([^"]+)"/g)].map((m) => m[1]))
			: null;
		page_ids.set(page, ids);
	}
	return ids;
}

/** The dead renders in one crate's rendered pages, by kind. */
interface DeadRenders {
	anchors: string[];
	pages: string[];
	inert: string[];
}

/** Reads `crate_name`'s rendered pages back (module doc §Rendered links); `graded` names
 * every crate whose pages a cross-crate href is checked against. `null` when the render
 * holds no `index.html`, which leaves nothing to read back and must not read as clean. */
function check_rendered(crate_name: string, graded: Set<string>): DeadRenders | null {
	const doc_root = join(TARGET_DIR, 'doc');
	const crate_dir = render_dir(crate_name);
	const dead: DeadRenders = { anchors: [], pages: [], inert: [] };
	if (!existsSync(join(crate_dir, 'index.html'))) return null;
	const pages = (readdirSync(crate_dir, { recursive: true }) as string[])
		.filter((f) => f.endsWith('.html'))
		.sort();
	for (const rel of pages) {
		const page = join(crate_dir, rel);
		const html = readFileSync(page, 'utf8');
		for (const m of html.matchAll(/\[<code>([^<]*)<\/code>\]/g)) {
			dead.inert.push(`${crate_name}/${rel}: [${m[1]}]`);
		}
		for (const m of html.matchAll(/\shref="([^"]*)"/g)) {
			const href = m[1];
			if (href === '' || href === '#' || href.includes('${')) continue;
			if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith('/')) continue;
			const hash = href.indexOf('#');
			const path = hash === -1 ? href : href.slice(0, hash);
			const fragment = hash === -1 ? '' : href.slice(hash + 1);
			const target = path === '' ? page : normalize(join(dirname(page), path));
			const inside = target.slice(doc_root.length + 1).split('/')[0];
			if (!target.startsWith(doc_root + '/') || !graded.has(inside)) continue;
			const ids = ids_of(target);
			if (ids === null) dead.pages.push(`${crate_name}/${rel} → ${href}`);
			else if (fragment !== '' && !ids.has(fragment))
				dead.anchors.push(`${crate_name}/${rel} → ${href}`);
		}
	}
	return dead;
}

function run(): never {
	const crates = docs_rs_crates();
	if (crates.length === 0) {
		log('Error: no workspace crate carries a [package.metadata.docs.rs] table — nothing graded.');
		process.exit(1);
	}

	let failed = 0;
	let builds = 0;
	// the crates whose docs.rs leg rendered, read back once every build has run
	const rendered = new Set<string>();
	for (const c of crates) {
		const docs_rs_shape: Array<string> = [];
		if (c.all_features) docs_rs_shape.push('--all-features');
		if (c.no_default_features) docs_rs_shape.push('--no-default-features');
		if (c.features.length > 0) docs_rs_shape.push('--features', c.features.join(','));
		// the defaults leg (module doc §The default-features leg) runs FIRST, so the render left
		// on disk is the docs.rs one (module doc §Rendered links)
		const legs: Array<[label: string, flags: Array<string>]> = [];
		if (docs_rs_shape.length > 0) legs.push(['default-features', []]);
		legs.push(['docs.rs', docs_rs_shape]);
		for (const [label, flags] of legs) {
			builds++;
			// module doc §Stale pages: this leg's render is all the directory holds
			rmSync(render_dir(c.name), { recursive: true, force: true });
			const result = spawnSync('cargo', ['doc', '-p', c.name, '--no-deps', '--quiet', ...flags], {
				encoding: 'utf8',
				env: { ...process.env, RUSTDOCFLAGS, CARGO_TARGET_DIR: TARGET_DIR },
				stdio: ['ignore', 'ignore', 'pipe']
			});
			const stderr = result.stderr ?? '';
			const shape = flags.length > 0 ? ` ${flags.join(' ')}` : '';
			if (result.status !== 0) {
				failed++;
				log(stderr.trimEnd());
				log(`✗ ${c.name}${shape}: the ${label} build fails`);
			} else {
				log(`✓ ${c.name}${shape} (${label})`);
				if (label === 'docs.rs') rendered.add(c.name);
			}
		}
	}

	// the read-back (module doc §Rendered links), after every build
	let dead_total = 0;
	let unreadable = 0;
	for (const name of rendered) {
		const dead = check_rendered(name, rendered);
		if (dead === null) {
			unreadable++;
			log(`✗ ${name}: the docs.rs render left no index.html to read back`);
			continue;
		}
		const count = dead.anchors.length + dead.pages.length + dead.inert.length;
		dead_total += count;
		if (count > 0) {
			log(
				`✗ ${name}: ${count} dead render(s) — ${dead.anchors.length} dead anchor(s), ` +
					`${dead.pages.length} dead page(s), ${dead.inert.length} inert`
			);
			if (verbose || count <= LIST_LIMIT) {
				for (const line of [...dead.anchors, ...dead.pages, ...dead.inert]) log(`    ${line}`);
			}
		}
	}

	if (failed > 0 || unreadable > 0 || dead_total > 0) {
		log(
			`docs:audit:docsrs FAILED — ${failed} of ${builds} build(s) failed, ${unreadable} ` +
				`render(s) left nothing to read back, ${dead_total} dead render(s) (a hidden or ` +
				'private link target; -v lists them)'
		);
		process.exit(1);
	}
	log(`docs:audit:docsrs OK — ${crates.length} crate(s), ${builds} build(s), no dead renders`);
	process.exit(0);
}

run();
