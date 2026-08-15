/**
 * Node.js tests for the built npm packages (@fuzdev/tsv_format_wasm,
 * @fuzdev/tsv_parse_wasm, and @fuzdev/tsv_wasm).
 *
 * Verifies the wasm-pack web target + patch_npm_package.ts wrapper works
 * correctly when imported as ESM in Node.js: the auto-init node entry
 * (index.js), the guarded browser entry (browser.js), the package.json
 * exports/files wiring, the bundled `locations.js` helper, the shared
 * discovery-parity suite over `IgnoreStack`, and (for the `all` variant) the
 * `tsv` bin.
 *
 * Runs under Node (not Deno) on purpose — it validates the package in the
 * runtime consumers use. Node's native type stripping executes the `.ts`
 * directly (requires Node >= 22.18; erasable syntax only).
 *
 * Usage: PKG_DIR=<pkg-dir> node --test scripts/test_npm.ts
 *
 * Examples:
 *   PKG_DIR=crates/tsv_wasm/pkg/format/npm node --test scripts/test_npm.ts
 *   PKG_DIR=crates/tsv_wasm/pkg/parse/npm node --test scripts/test_npm.ts
 *   PKG_DIR=crates/tsv_wasm/pkg/all/npm node --test scripts/test_npm.ts
 *
 * Prerequisites: deno task build:npm:format (or build:npm:parse / build:npm:all)
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
	existsSync,
	mkdirSync,
	mkdtempSync,
	readdirSync,
	readFileSync,
	rmSync,
	writeFileSync
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

import { register_discovery_parity_suite } from './discovery_parity_suite.ts';

const pkg_dir = process.env.PKG_DIR;
if (!pkg_dir) {
	console.error('Usage: PKG_DIR=<pkg-dir> node --test scripts/test_npm.ts');
	console.error('Example: PKG_DIR=crates/tsv_wasm/pkg/format/npm node --test scripts/test_npm.ts');
	process.exit(1);
}

const variant = /\/(format|parse|all)\/npm\/?$/.exec(pkg_dir)?.[1] as
	'format' | 'parse' | 'all' | undefined;
if (!variant) {
	console.error(`PKG_DIR must end in /(format|parse|all)/npm — got ${pkg_dir}`);
	process.exit(1);
}
const has_format = variant !== 'parse';
const has_parse = variant !== 'format';
const PKG_NAMES = {
	format: '@fuzdev/tsv_format_wasm',
	parse: '@fuzdev/tsv_parse_wasm',
	all: '@fuzdev/tsv_wasm'
};

const node_entry = await import(`../${pkg_dir}/index.js`);

/** Repo root — `PKG_DIR` is repo-relative, and this file lives in `scripts/`. */
const repo_root = dirname(import.meta.dirname);

/** Structural equality for plain wire JSON (no cycles, no undefined-vs-missing nuance). */
const deep_equal_json = (a: unknown, b: unknown): boolean =>
	JSON.stringify(a) === JSON.stringify(b);

describe(`package metadata: ${pkg_dir}`, () => {
	const pkg = JSON.parse(
		readFileSync(new URL(`../${pkg_dir}/package.json`, import.meta.url), 'utf-8')
	);

	it('has the right name', () => {
		assert.equal(pkg.name, PKG_NAMES[variant]);
	});

	it('exports map points at files that exist', () => {
		const root = pkg.exports['.'];
		for (const key of ['types', 'node', 'default']) {
			const rel = root[key];
			assert.ok(rel, `exports['.'].${key} missing`);
			assert.ok(
				existsSync(new URL(`../${pkg_dir}/${rel}`, import.meta.url)),
				`exports['.'].${key} → ${rel} does not exist`
			);
		}
	});

	it('every files[] entry exists', () => {
		for (const rel of pkg.files) {
			assert.ok(
				existsSync(new URL(`../${pkg_dir}/${rel}`, import.meta.url)),
				`files entry ${rel} does not exist`
			);
		}
	});

	it('index.js is marked side-effectful (auto-init survives tree-shaking)', () => {
		assert.deepEqual(pkg.sideEffects, ['./index.js']);
	});

	it('parse-capable variants bundle tsv_ast.d.ts', { skip: !has_parse }, () => {
		assert.ok(pkg.files.includes('tsv_ast.d.ts'));
	});

	it('parse-capable variants bundle the locations helper', { skip: !has_parse }, () => {
		assert.ok(pkg.files.includes('locations.js'));
		assert.ok(pkg.files.includes('locations.d.ts'));
	});

	it('all variant ships the tsv bin', { skip: variant !== 'all' }, () => {
		assert.deepEqual(pkg.bin, { tsv: './cli.js' });
		assert.ok(pkg.files.includes('cli.js'));
	});

	it('subset variants ship no bin', { skip: variant === 'all' }, () => {
		assert.equal(pkg.bin, undefined);
	});
});

describe(`node entry (index.js): ${pkg_dir}`, () => {
	it('format_typescript formats', { skip: !has_format }, () => {
		assert.equal(node_entry.format_typescript('const   x=1'), 'const x = 1;\n');
	});

	it('format_css formats', { skip: !has_format }, () => {
		assert.equal(node_entry.format_css('a{color:red}'), 'a {\n\tcolor: red;\n}\n');
	});

	it('format_svelte formats', { skip: !has_format }, () => {
		assert.equal(node_entry.format_svelte('<div   >x</div   >'), '<div>x</div>\n');
	});

	it('formatting is idempotent', { skip: !has_format }, () => {
		const once = node_entry.format_svelte('<script>const   x=1</script>\n\n<div>{x}</div>');
		assert.equal(node_entry.format_svelte(once), once);
	});

	it('throws a useful error on invalid syntax', { skip: !has_format }, () => {
		assert.throws(() => node_entry.format_typescript('const ='));
	});

	it('format options: goal switches the TypeScript parse goal', { skip: !has_format }, () => {
		// at the script goal `await` is an ordinary identifier, so it parses as an
		// arrow parameter (and prints parenthesized)
		assert.equal(
			node_entry.format_typescript('await => 1;', { goal: 'script' }),
			'(await) => 1;\n'
		);
		// module goal (default and explicit) reserves `await`
		assert.throws(() => node_entry.format_typescript('await => 1;'));
		assert.throws(() => node_entry.format_typescript('await => 1;', { goal: 'module' }));
		assert.throws(() => node_entry.format_typescript('x;', { goal: 'bogus' }), /invalid goal/);
		assert.throws(
			() => node_entry.format_typescript('x;', { goal: 42 }),
			/'goal' must be 'script' or 'module'/
		);
	});

	it('format options: unknown keys and misapplied goal error', { skip: !has_format }, () => {
		assert.throws(
			() => node_entry.format_typescript('x;', { gaol: 'script' }),
			/unknown format option 'gaol'/
		);
		// `locations` shapes the parse WIRE and format emits none, so it is an
		// unknown key here rather than an accepted-and-inert one — an inert
		// spelling would let a caller believe they had asked a formatter for the
		// narrower product
		assert.throws(
			() => node_entry.format_typescript('x;', { locations: false }),
			/unknown format option 'locations'/
		);
		// svelte/css formatting is non-configurable and the goal is TypeScript's
		// alone, so their bags carry no key at all
		assert.throws(
			() => node_entry.format_svelte('<div>x</div>', { goal: 'script' }),
			/only supported for TypeScript/
		);
		assert.throws(
			() => node_entry.format_svelte('<div>x</div>', { locations: false }),
			/takes no options/
		);
		// a supported key explicitly set to `undefined` means that key's default
		// (omitted-key convention) — including the TS-only key on a language that
		// REJECTS it, which is what lets `npm/cli.js` hand one bag to whichever
		// formatter instead of branching the call
		assert.equal(
			node_entry.format_typescript('const   x=1', { goal: undefined }),
			'const x = 1;\n'
		);
		assert.equal(
			node_entry.format_svelte('<div   >x</div   >', { goal: undefined }),
			'<div>x</div>\n'
		);
		assert.equal(
			node_entry.format_css('a{color:red}', { goal: undefined }),
			'a {\n\tcolor: red;\n}\n'
		);
		// an UNKNOWN key throws even at `undefined` — the typo guard has no
		// undefined-valued hole
		assert.throws(
			() => node_entry.format_typescript('x;', { gaol: undefined }),
			/unknown format option 'gaol'/
		);
		// a non-object options argument is an error, arrays included
		assert.throws(() => node_entry.format_typescript('x;', 'script'), /must be an object/);
		assert.throws(() => node_entry.format_typescript('x;', ['script']), /must be an object/);
		// `null` and `undefined` both mean all-defaults — `null` is the arm that
		// would otherwise fall through to the non-object error, since it is
		// `typeof 'object'` — and so does `{}`, the zero-key object path
		assert.equal(node_entry.format_typescript('const   x=1', null), 'const x = 1;\n');
		assert.equal(node_entry.format_typescript('const   x=1', undefined), 'const x = 1;\n');
		assert.equal(node_entry.format_typescript('const   x=1', {}), 'const x = 1;\n');
	});

	it('format_* absent from the parse-only build', { skip: has_format }, () => {
		assert.equal(node_entry.format_typescript, undefined);
	});

	it('parse_* absent from the format-only build', { skip: has_parse }, () => {
		assert.equal(node_entry.parse_typescript, undefined);
	});

	it('reconstruct_locations absent from the format-only build', { skip: has_parse }, () => {
		assert.equal(node_entry.reconstruct_locations, undefined);
		assert.equal(node_entry.create_locator, undefined);
	});

	it('parse_typescript returns a Program', { skip: !has_parse }, () => {
		const program = node_entry.parse_typescript('const x = 1;');
		assert.equal(program.type, 'Program');
		assert.ok(Array.isArray(program.body));
	});

	it('parse_typescript_json returns a JSON string', { skip: !has_parse }, () => {
		const json = node_entry.parse_typescript_json('const x = 1;');
		assert.equal(typeof json, 'string');
		assert.equal(JSON.parse(json).type, 'Program');
	});

	it('parse_svelte and parse_css work', { skip: !has_parse }, () => {
		assert.equal(node_entry.parse_svelte('<div>x</div>').type, 'Root');
		assert.equal(node_entry.parse_css('a { color: red }').type, 'StyleSheetFile');
	});

	it('parse_*_json round-trips to parse_*', { skip: !has_parse }, () => {
		// parse_X is defined as JSON.parse(parse_X_json(src)); the two must agree.
		assert.deepEqual(
			JSON.parse(node_entry.parse_typescript_json('const x = 1;')),
			node_entry.parse_typescript('const x = 1;')
		);
		assert.deepEqual(
			JSON.parse(node_entry.parse_svelte_json('<div>x</div>')),
			node_entry.parse_svelte('<div>x</div>')
		);
		assert.deepEqual(
			JSON.parse(node_entry.parse_css_json('a { color: red }')),
			node_entry.parse_css('a { color: red }')
		);
	});

	it('parse options: {locations: false} emits the span-only wire', { skip: !has_parse }, () => {
		const full = node_entry.parse_typescript('const x = 1;');
		const span_only = node_entry.parse_typescript('const x = 1;', { locations: false });
		assert.ok(full.loc);
		assert.equal('loc' in span_only, false);
		assert.equal(span_only.type, 'Program');
		// svelte also drops name_loc
		const sv = '<div class="a">x</div>';
		assert.match(JSON.stringify(node_entry.parse_svelte(sv)), /"name_loc"/);
		const sv_span_only = JSON.stringify(node_entry.parse_svelte(sv, { locations: false }));
		assert.doesNotMatch(sv_span_only, /"name_loc"|"loc"/);
		// css accepts the option as an inert no-op (its wire has no loc either way)
		assert.deepEqual(
			node_entry.parse_css('a { color: red }', { locations: false }),
			node_entry.parse_css('a { color: red }')
		);
	});

	it('parse options: goal switches the TypeScript parse goal', { skip: !has_parse }, () => {
		const program = node_entry.parse_typescript('var await = 1;', { goal: 'script' });
		assert.equal(program.sourceType, 'script');
		// module goal (default and explicit) reserves `await`
		assert.throws(() => node_entry.parse_typescript('var await = 1;'));
		assert.throws(() => node_entry.parse_typescript('var await = 1;', { goal: 'module' }));
		assert.throws(() => node_entry.parse_typescript('x;', { goal: 'bogus' }), /invalid goal/);
		// goal composes with locations (goal drives the parser, locations the writer)
		const composed = node_entry.parse_typescript('var await = 1;', {
			goal: 'script',
			locations: false
		});
		assert.equal(composed.sourceType, 'script');
		assert.equal('loc' in composed, false);
	});

	it('parse options: unknown keys and misapplied goal error', { skip: !has_parse }, () => {
		assert.throws(
			() => node_entry.parse_typescript('x;', { locatons: false }),
			/unknown parse option 'locatons'/
		);
		assert.throws(
			() => node_entry.parse_svelte('<div>x</div>', { goal: 'script' }),
			/only supported for TypeScript/
		);
		assert.throws(
			() => node_entry.parse_typescript('x;', { locations: 'yes' }),
			/'locations' must be a boolean/
		);
		// a supported key explicitly set to undefined means that key's default
		// (omitted-key convention)
		assert.ok(node_entry.parse_typescript('x;', { locations: undefined, goal: undefined }).loc);
		// ...including the TS-only key on a language that REJECTS it. Load-bearing:
		// `npm/cli.js` forwards one options bag to whichever parser and spells the
		// inapplicable goal as `undefined` rather than branching the call. The goal
		// arm must read `undefined` before its language rejection, or this breaks
		// with `check` still green. `ParseOptions` declares `goal?: undefined` so
		// the same bag type-checks; see ../crates/tsv_wasm/CLAUDE.md.
		assert.ok(node_entry.parse_svelte('<div>x</div>', { goal: undefined }));
		assert.ok(node_entry.parse_css('a { color: red }', { goal: undefined }));
		// an UNKNOWN key throws even at `undefined` — the typo guard has no
		// undefined-valued hole; only supported keys read `undefined` as absent
		assert.throws(
			() => node_entry.parse_typescript('x;', { locatons: undefined }),
			/unknown parse option 'locatons'/
		);
		// a non-object options argument is an error, arrays included
		assert.throws(() => node_entry.parse_typescript('x;', 'locations'), /must be an object/);
		assert.throws(() => node_entry.parse_typescript('x;', []), /must be an object/);
		// `null` and `undefined` both mean all-defaults — `null` is the arm that
		// would otherwise fall through to the non-object error, since it is
		// `typeof 'object'` — and so does `{}`, the zero-key object path
		assert.ok(node_entry.parse_typescript('x;', null).loc);
		assert.ok(node_entry.parse_typescript('x;', undefined).loc);
		assert.ok(node_entry.parse_typescript('x;', {}).loc);
	});
});

// Locations helper (locations.js, re-exported from index.js) — reconstruct the
// per-node `loc` a `no-locations` wire drops, from `start`/`end` + source. Its
// correctness is also gated by benches/js/diagnostics/no_locations_parity.ts at
// corpus scale; these assert the shipped package export works end-to-end.
describe(`locations helper (index.js): ${pkg_dir}`, { skip: !has_parse }, () => {
	it('reconstruct_locations is EXACT for TypeScript (equals the full wire)', () => {
		const ts = 'const x = 1;\nconst y = 2;\n';
		const full = node_entry.parse_typescript(ts);
		const recon = node_entry.reconstruct_locations(
			node_entry.parse_typescript(ts, { locations: false }),
			ts
		);
		// The span-only wire is the full wire minus `loc`; adding it back must
		// reproduce acorn's `loc` byte-for-byte on every node.
		assert.deepEqual(recon, full);
	});

	it('reconstruct_locations mutates in place and returns the same object', () => {
		const ast = node_entry.parse_typescript('const x = 1;', { locations: false });
		const returned = node_entry.reconstruct_locations(ast, 'const x = 1;');
		assert.equal(returned, ast); // same reference
		assert.ok(ast.loc); // mutated in place
	});

	it('loc_of derives a single node line/column', () => {
		const ts = 'const x = 1;\nconst y = 2;\n';
		const full = node_entry.parse_typescript(ts);
		const noloc = node_entry.parse_typescript(ts, { locations: false });
		// second statement starts on line 2, column 0
		assert.deepEqual(node_entry.loc_of(noloc.body[1], ts), full.body[1].loc);
		assert.equal(node_entry.loc_of({ type: 'X' }, ts), null); // no start/end → null
	});

	it('create_locator reuses one line table for many lookups', () => {
		const ts = 'const x = 1;\nconst y = 2;\n';
		const full = node_entry.parse_typescript(ts);
		const noloc = node_entry.parse_typescript(ts, { locations: false });
		const locator = node_entry.create_locator(ts);
		assert.deepEqual(locator.loc_of(noloc.body[0]), full.body[0].loc);
		assert.deepEqual(locator.loc_of(noloc.body[1]), full.body[1].loc);
	});

	it('reconstruct_locations is a no-op for CSS (parseCss emits no loc)', () => {
		// CSS has no `loc` in the wire, so reconstruct must leave the tree untouched.
		// Language inferred from the `StyleSheetFile` root.
		const css = node_entry.parse_css('a {\n\tcolor: red;\n}\n');
		const out = node_entry.reconstruct_locations(css, 'a {\n\tcolor: red;\n}\n');
		assert.equal(out, css);
		assert.equal('loc' in css, false); // root gained no loc
		assert.equal('loc' in css.children[0], false); // nor did a rule node
	});

	it('reconstruct_locations matches Svelte acorn-node loc, except the documented quirks', () => {
		// No destructure pattern here, so the Svelte +1-column quirk can't appear;
		// the `<script>` Program loc (Svelte's tag-position override) is skipped.
		// Covers each `name_loc` shape (tag name, attribute, shorthand padded and
		// not, a directive head with modifiers) and each identifier Svelte gives the
		// name-shaped `loc` (shorthand expansion, snippet name, block patterns).
		const sv =
			'<script>\nconst x = 1;\n</script>\n\n<div class="a" on:click|preventDefault={x} {x} { x }>\n\t<svelte:head><title>t</title></svelte:head>\n\t{@const y = x}\n\t{#each [x] as item}{item}{/each}\n\t{#await x}p{:then value}{value}{:catch err}{err}{/await}\n</div>\n{#snippet row(a)}{a}{/snippet}';
		const full = node_entry.parse_svelte(sv);
		const recon = node_entry.reconstruct_locations(
			node_entry.parse_svelte(sv, { locations: false }),
			sv
		);
		let checked = 0;
		let name_locs_checked = 0;
		const walk = (r: any, f: any): void => {
			if (Array.isArray(f)) {
				f.forEach((x, i) => walk(r[i], x));
			} else if (f && typeof f === 'object') {
				// Svelte's wire carries `loc` only on embedded ECMAScript nodes; where
				// it does, the reconstruction must match — except `Program`.
				if (f.loc && f.type !== 'Program') {
					assert.deepEqual(r.loc, f.loc, `loc mismatch at ${f.type}@${f.start}`);
					checked++;
				}
				// `name_loc` (elements, attributes, directives) is exact — its span is a
				// function of the node's own start/end + type.
				if (f.name_loc) {
					assert.deepEqual(r.name_loc, f.name_loc, `name_loc mismatch at ${f.type}@${f.start}`);
					name_locs_checked++;
				}
				for (const k of Object.keys(f)) {
					if (k === 'loc' || k === 'name_loc') continue;
					walk(r[k], f[k]);
				}
			}
		};
		walk(recon, full);
		assert.ok(checked > 0, 'expected at least one acorn node to compare');
		assert.ok(name_locs_checked > 0, 'expected at least one name_loc to compare');
		// The walk is a superset: it adds `loc` to template nodes Svelte's wire omits.
		assert.ok(recon.loc, 'reconstruct added loc to the Root (template node)');
	});

	// The hand-written case above pins the shapes a reader can follow; this one drives
	// the SAME helper over every `.svelte` fixture in the repo, so the tables it carries
	// (`NAME_LOC_KINDS`, the character-bearing shapes) are graded against what the writer
	// actually emits rather than against a remembered list. Every mismatch must fall into
	// one of the two documented Svelte divergences — anything else fails.
	//
	// Self-oracle by construction: it grades `reconstruct(no-loc) == full wire`, so it
	// proves reconstruction fidelity, NOT conformance to Svelte. The conformance oracle is
	// `deno task corpus:compare:parse` plus the root Rust span tests.
	it('reconstructs every .svelte fixture, modulo the two documented quirks', () => {
		const roots = [join(repo_root, 'tests/fixtures'), join(repo_root, 'tests/fixtures_compile')];
		const files: Array<string> = [];
		const collect = (dir: string): void => {
			for (const entry of readdirSync(dir, { withFileTypes: true })) {
				const p = join(dir, entry.name);
				if (entry.isDirectory()) collect(p);
				else if (entry.name.endsWith('.svelte')) files.push(p);
			}
		};
		for (const r of roots) if (existsSync(r)) collect(r);
		assert.ok(files.length > 500, `expected the fixture tree, found ${files.length} .svelte files`);

		const findings: Array<string> = [];
		let compared = 0;
		let character_locs = 0;
		let scanned = 0;

		const compare = (recon: any, full: any, file: string): void => {
			if (Array.isArray(full)) {
				full.forEach((x, i) => compare(recon?.[i], x, file));
				return;
			}
			if (!full || typeof full !== 'object') return;
			if (full.name_loc) {
				compared++;
				if (!deep_equal_json(recon?.name_loc, full.name_loc)) {
					findings.push(`${file}: name_loc on ${full.type}@${full.start}`);
				}
			}
			if (full.loc) {
				compared++;
				const has_character = 'character' in (full.loc.start ?? {});
				if (has_character) character_locs++;
				if (!deep_equal_json(recon?.loc, full.loc)) {
					// documented quirk 1: Svelte's `<script>`/`<style>` tag-position override
					const script_program = full.type === 'Program';
					// documented quirk 2: the destructure-pattern synthetic-`(` column shift
					const destructure_shift =
						recon?.loc?.start?.line === full.loc.start.line &&
						full.loc.start.column - (recon?.loc?.start?.column ?? 0) === 1;
					if (!script_program && !destructure_shift) {
						findings.push(`${file}: loc on ${full.type}@${full.start}`);
					}
				}
			}
			for (const k of Object.keys(full)) {
				if (k === 'loc' || k === 'name_loc') continue;
				compare(recon?.[k], full[k], file);
			}
		};

		for (const file of files) {
			const source = readFileSync(file, 'utf8');
			let full;
			let span_only;
			try {
				full = node_entry.parse_svelte(source);
				span_only = node_entry.parse_svelte(source, { locations: false });
			} catch {
				continue; // a fixture tsv rejects on purpose (input_invalid_*, tsv_rejects)
			}
			scanned++;
			const recon = node_entry.reconstruct_locations(span_only, source, { language: 'svelte' });
			compare(recon, full, file.slice(repo_root.length + 1));
		}

		assert.ok(scanned > 500, `expected to parse most fixtures, parsed ${scanned}`);
		assert.ok(compared > 10_000, `expected a broad comparison, made ${compared}`);
		assert.ok(character_locs > 0, 'expected some character-bearing locs (in-tag comments)');
		assert.deepEqual(findings.slice(0, 10), [], `${findings.length} undocumented mismatches`);
	});
});

// Browser entry (browser.js) — tests the init guard wrapper.
// Imports browser.js which does NOT auto-init WASM, then tests:
// - Pre-init guard throws a clear error
// - Post-init_sync: format functions work, init is idempotent
describe(`browser entry (browser.js): ${pkg_dir}`, () => {
	let browser: any;

	it('import browser.js', async () => {
		browser = await import(`../${pkg_dir}/browser.js`);
	});

	it('exports throw before init', () => {
		const guarded = has_format ? 'format_typescript' : 'parse_typescript';
		assert.throws(() => browser[guarded]('const x = 1'), /WASM not initialized/);
	});

	it('reconstruct_locations works BEFORE init (pure JS, no WASM)', { skip: !has_parse }, () => {
		// A hand-built span-only node — no parse needed, so this runs before the
		// init_sync below, proving the helper carries no init guard (it never
		// touches WASM). This is why browser.js re-exports it directly.
		const ast = { type: 'Program', start: 0, end: 5, body: [] };
		const out = browser.reconstruct_locations(ast, 'x = 1');
		assert.deepEqual(out.loc, {
			start: { line: 1, column: 0 },
			end: { line: 1, column: 5 }
		});
	});

	it('init_sync initializes WASM', () => {
		const wasm = readFileSync(new URL(`../${pkg_dir}/tsv_wasm_bg.wasm`, import.meta.url));
		browser.init_sync({ module: wasm });
	});

	it('format functions work after init', { skip: !has_format }, () => {
		assert.equal(browser.format_typescript('const   x=1'), 'const x = 1;\n');
		assert.equal(browser.format_css('a{color:red}'), 'a {\n\tcolor: red;\n}\n');
		assert.equal(browser.format_svelte('<div   >x</div   >'), '<div>x</div>\n');
	});

	it('parse works after init', { skip: !has_parse }, () => {
		assert.equal(browser.parse_typescript('const x = 1;').type, 'Program');
	});

	// Regression: the guard wrappers once hardcoded a single `(source)` param,
	// silently dropping every later argument in the browser entry only.
	it('the init guard forwards extra args (parse options)', { skip: !has_parse }, () => {
		const program = browser.parse_typescript('var await = 1;', {
			goal: 'script',
			locations: false
		});
		assert.equal(program.sourceType, 'script');
		assert.equal('loc' in program, false);
	});

	it('the init guard forwards extra args (format options)', { skip: !has_format }, () => {
		assert.equal(browser.format_typescript('await => 1;', { goal: 'script' }), '(await) => 1;\n');
	});

	it('init is idempotent after init_sync', async () => {
		// Should resolve without re-fetching — just returns early
		await browser.init();
		if (has_format) {
			assert.equal(browser.format_typescript('const   x=1'), 'const x = 1;\n');
		} else {
			assert.equal(browser.parse_typescript('const x = 1;').type, 'Program');
		}
	});
});

// CLI (`tsv` bin, `all` variant only) — subprocess tests against the contract
// the JS CLI mirrors from the native tsv_cli: flags, exit codes, output streams.
describe(`cli (cli.js): ${pkg_dir}`, { skip: variant !== 'all' }, () => {
	const cli_path = new URL(`../${pkg_dir}/cli.js`, import.meta.url).pathname;
	const run_cli = (args: Array<string>, stdin?: string, cwd?: string) =>
		spawnSync(process.execPath, [cli_path, ...args], {
			encoding: 'utf-8',
			input: stdin,
			cwd
		});

	it('format --content prints formatted source', () => {
		const result = run_cli(['format', '--content', 'const   x=1', '--parser', 'typescript']);
		assert.equal(result.status, 0);
		assert.equal(result.stdout, 'const x = 1;\n');
	});

	it('--parser accepts the ts alias', () => {
		const result = run_cli(['format', '--content', 'const   x=1', '--parser', 'ts']);
		assert.equal(result.status, 0);
		assert.equal(result.stdout, 'const x = 1;\n');
	});

	it('format --stdin reads stdin', () => {
		const result = run_cli(['format', '--stdin', '--parser', 'css'], 'a{color:red}');
		assert.equal(result.status, 0);
		assert.equal(result.stdout, 'a {\n\tcolor: red;\n}\n');
	});

	it('format --content without --parser exits 2', () => {
		const result = run_cli(['format', '--content', 'const x = 1;']);
		assert.equal(result.status, 2);
		assert.match(result.stderr, /requires --parser/);
	});

	it('format --check --content exits 1 on would-change', () => {
		const result = run_cli(['format', '--check', '--content', 'const   x=1', '--parser', 'ts']);
		assert.equal(result.status, 1);
		assert.match(result.stderr, /would change/);
	});

	it('format --check --content exits 0 on clean input', () => {
		const result = run_cli(['format', '--check', '--content', 'const x = 1;\n', '--parser', 'ts']);
		assert.equal(result.status, 0);
	});

	it('format on invalid syntax exits 2', () => {
		const result = run_cli(['format', '--content', 'const =', '--parser', 'ts']);
		assert.equal(result.status, 2);
		assert.match(result.stderr, /Parse error/);
	});

	it('unknown flags exit 1', () => {
		const result = run_cli(['format', '--bogus']);
		assert.equal(result.status, 1);
	});

	it('a bad --parser value exits 1 in both commands (argument-parsing error)', () => {
		for (const command of ['format', 'parse']) {
			const result = run_cli([command, '--content', 'const x = 1;', '--parser', 'bogus']);
			assert.equal(result.status, 1);
			assert.match(result.stderr, /Unknown parser type/);
		}
	});

	it('parse --goal script accepts `await` as an identifier; module/default reject it', () => {
		const script = run_cli([
			'parse',
			'--content',
			'var await = 1;',
			'--parser',
			'ts',
			'--goal',
			'script'
		]);
		assert.equal(script.status, 0, script.stderr);
		assert.match(script.stdout, /"sourceType":"script"/);

		// the same source is reserved at Module goal (explicit and default)
		const mod = run_cli([
			'parse',
			'--content',
			'var await = 1;',
			'--parser',
			'ts',
			'--goal',
			'module'
		]);
		assert.equal(mod.status, 1);
		const dflt = run_cli(['parse', '--content', 'var await = 1;', '--parser', 'ts']);
		assert.equal(dflt.status, 1);
	});

	it('parse --no-locations omits per-node loc and composes with --goal', () => {
		const bare = run_cli([
			'parse',
			'--no-locations',
			'--content',
			'const x = 1;',
			'--parser',
			'ts'
		]);
		assert.equal(bare.status, 0, bare.stderr);
		assert.doesNotMatch(bare.stdout, /"loc"/);
		const composed = run_cli([
			'parse',
			'--no-locations',
			'--goal',
			'script',
			'--content',
			'var await = 1;',
			'--parser',
			'ts'
		]);
		assert.equal(composed.status, 0, composed.stderr);
		assert.match(composed.stdout, /"sourceType":"script"/);
		assert.doesNotMatch(composed.stdout, /"loc"/);
		// svelte drops name_loc too; css accepts the flag as a no-op
		const sv = run_cli([
			'parse',
			'--no-locations',
			'--content',
			'<div>x</div>',
			'--parser',
			'svelte'
		]);
		assert.equal(sv.status, 0, sv.stderr);
		assert.doesNotMatch(sv.stdout, /"name_loc"|"loc"/);
		const css = run_cli([
			'parse',
			'--no-locations',
			'--content',
			'a{color:red}',
			'--parser',
			'css'
		]);
		assert.equal(css.status, 0, css.stderr);
	});

	it('format --goal script formats an `await` arrow param', () => {
		const result = run_cli([
			'format',
			'--content',
			'await => 1;',
			'--parser',
			'ts',
			'--goal',
			'script'
		]);
		assert.equal(result.status, 0, result.stderr);
		assert.equal(result.stdout, '(await) => 1;\n');
	});

	it('an invalid --goal value exits 1 for parse, 2 for format (arg-error parity)', () => {
		const p = run_cli(['parse', '--content', 'x;', '--parser', 'ts', '--goal', 'bogus']);
		assert.equal(p.status, 1);
		assert.match(p.stderr, /invalid --goal/);
		const f = run_cli(['format', '--content', 'x;', '--parser', 'ts', '--goal', 'bogus']);
		assert.equal(f.status, 2);
		assert.match(f.stderr, /invalid --goal/);
	});

	it('format --goal with a path argument is a usage error (exit 2)', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'a.ts'), 'const x = 1;\n');
			const result = run_cli(['format', '--goal', 'script', join(dir, 'a.ts')]);
			assert.equal(result.status, 2);
			assert.match(result.stderr, /--goal applies to --content\/--stdin/);
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('format paths writes in place, recurses, and skips excluded dirs', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'a.ts'), 'const   x=1');
			writeFileSync(join(dir, 'clean.css'), 'a {\n\tcolor: red;\n}\n');
			mkdirSync(join(dir, 'nested'));
			writeFileSync(join(dir, 'nested', 'b.svelte'), '<div   >x</div   >');
			mkdirSync(join(dir, 'node_modules'));
			writeFileSync(join(dir, 'node_modules', 'skip.ts'), 'const   y=2');

			const result = run_cli(['format', dir]);
			assert.equal(result.status, 0);
			assert.deepEqual(result.stdout.trim().split('\n').sort(), [
				join(dir, 'a.ts'),
				join(dir, 'nested', 'b.svelte')
			]);
			assert.match(result.stderr, /2 formatted, 1 unchanged/);
			assert.equal(readFileSync(join(dir, 'a.ts'), 'utf-8'), 'const x = 1;\n');
			assert.equal(readFileSync(join(dir, 'nested', 'b.svelte'), 'utf-8'), '<div>x</div>\n');
			assert.equal(readFileSync(join(dir, 'node_modules', 'skip.ts'), 'utf-8'), 'const   y=2');

			// Second run: everything clean, --check passes.
			const check = run_cli(['format', '--check', dir]);
			assert.equal(check.status, 0);
			assert.match(check.stderr, /0 would change, 3 unchanged/);
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	// --list binary contract (read-only, exit codes, flag rejection). *Which*
	// files the ignore files admit is pinned for both CLIs by the shared table
	// in the `discovery parity` suite below; this only covers the --list contract.
	it('format --list is read-only, exits 0 on an empty scope, and rejects --check', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			mkdirSync(join(dir, '.git'));
			writeFileSync(join(dir, '.gitignore'), 'build/\n');
			writeFileSync(join(dir, 'a.ts'), 'const   x=1');
			mkdirSync(join(dir, 'build'));
			writeFileSync(join(dir, 'build', 'out.ts'), 'const x = 1;\n');

			const list = run_cli(['format', '--list', '.'], undefined, dir);
			assert.equal(list.status, 0, list.stderr);
			assert.match(list.stdout, /a\.ts/);
			// --list never writes — the unformatted file is left exactly as-is
			assert.equal(readFileSync(join(dir, 'a.ts'), 'utf-8'), 'const   x=1');

			// an all-ignored target lists nothing and still exits 0
			const empty = run_cli(['format', '--list', 'build'], undefined, dir);
			assert.equal(empty.status, 0, empty.stderr);
			assert.equal(empty.stdout.trim(), '');

			// --list and --check are mutually exclusive
			const both = run_cli(['format', '--list', '--check', '.'], undefined, dir);
			assert.equal(both.status, 2);
			assert.match(both.stderr, /--list and --check/);
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	// #5 diagnostic parity: the heuristic-shadow warning (mirrors the native
	// `heuristic_shadow_warning` text). Non-fatal — stderr only, exit 0, --list
	// stdout stays clean, build/ stays pruned.
	it('format warns when the heuristic prunes a dir an anchored `!` targets', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			// no .git -> heuristic regime; an anchored `!build/keep.ts` is a no-op
			mkdirSync(join(dir, 'build'));
			writeFileSync(join(dir, '.formatignore'), '!build/keep.ts\n');
			writeFileSync(join(dir, 'a.ts'), 'const x = 1;\n');
			writeFileSync(join(dir, 'build', 'keep.ts'), 'const x = 1;\n');

			const list = run_cli(['format', '--list', '.'], undefined, dir);
			assert.equal(list.status, 0, list.stderr);
			assert.match(list.stdout, /a\.ts/);
			assert.doesNotMatch(list.stdout, /keep\.ts/); // build/ still pruned
			assert.match(list.stderr, /build is skipped by tsv's build-output heuristic/);
			assert.match(list.stderr, /re-include the directory itself/);

			// a floating `!keep.ts` must NOT warn (targets any depth, not build/)
			writeFileSync(join(dir, '.formatignore'), '!keep.ts\n');
			const floating = run_cli(['format', '--list', '.'], undefined, dir);
			assert.equal(floating.status, 0, floating.stderr);
			assert.doesNotMatch(floating.stderr, /warning:/);

			// the dir-level escape `!build/` re-includes build/: no prune, no warning
			writeFileSync(join(dir, '.formatignore'), '!build/\n');
			const escape = run_cli(['format', '--list', '.'], undefined, dir);
			assert.equal(escape.status, 0, escape.stderr);
			assert.doesNotMatch(escape.stderr, /warning:/);
			assert.match(escape.stdout, /keep\.ts/); // now in scope
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('format scope is cwd-independent for a non-repo .formatignore', () => {
		const base = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			const proj = join(base, 'proj');
			mkdirSync(join(proj, 'gen'), { recursive: true });
			// no .git -> .formatignore governs from the filesystem root down
			writeFileSync(join(proj, '.formatignore'), 'gen/\n');
			writeFileSync(join(proj, 'src.ts'), 'const x = 1;\n');
			writeFileSync(join(proj, 'gen', 'out.ts'), 'const x = 1;\n');

			// (a) cd into proj and list '.'; (b) from base, list proj by path
			const inside = run_cli(['format', '--list', '.'], undefined, proj);
			const outside = run_cli(['format', '--list', proj], undefined, base);
			for (const [label, out] of [
				['inside', inside],
				['outside', outside]
			] as const) {
				assert.equal(out.status, 0, `${label}: ${out.stderr}`);
				assert.match(out.stdout, /src\.ts/, label);
				assert.doesNotMatch(out.stdout, /out\.ts/, `${label}: gen/ honored regardless of cwd`);
			}
		} finally {
			rmSync(base, { recursive: true, force: true });
		}
	});

	it('format --check exits 1 and leaves files untouched', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'a.ts'), 'const   x=1');
			const result = run_cli(['format', '--check', dir]);
			assert.equal(result.status, 1);
			assert.match(result.stderr, /1 would change/);
			assert.equal(readFileSync(join(dir, 'a.ts'), 'utf-8'), 'const   x=1');
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('format on a missing path exits 2', () => {
		const result = run_cli(['format', '/nonexistent/tsv-cli-test']);
		assert.equal(result.status, 2);
		assert.match(result.stderr, /not a file or directory/);
	});

	// An explicitly named file bypasses the ignore files but not the extension
	// check — otherwise the parser dispatch (no unknown arm) hands a `.json` file
	// to the TypeScript parser, which for a top-level-array JSON *succeeds* and
	// rewrites it into a TS expression statement. Mirrors the native CLI, message
	// and all, via `tsv_discover::unsupported_extension_error`.
	it('format on an explicit file with an unsupported extension exits 2 and writes nothing', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			const json = join(dir, 'list.json');
			writeFileSync(json, '[1,   2,    3]\n');
			const result = run_cli(['format', json]);
			assert.equal(result.status, 2);
			assert.match(result.stderr, /unsupported file extension/);
			assert.match(result.stderr, /\.svelte/);
			assert.equal(readFileSync(json, 'utf-8'), '[1,   2,    3]\n');
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	// A directory argument is a scope, not a target: its contents are filtered by
	// the walk, so an unsupported file there is skipped rather than failing the run.
	it('format on a directory skips unsupported extensions instead of failing', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'a.ts'), 'const   x=1');
			writeFileSync(join(dir, 'data.json'), '[1,   2,    3]\n');
			const result = run_cli(['format', dir]);
			assert.equal(result.status, 0);
			assert.equal(readFileSync(join(dir, 'a.ts'), 'utf-8'), 'const x = 1;\n');
			assert.equal(readFileSync(join(dir, 'data.json'), 'utf-8'), '[1,   2,    3]\n');
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('format --content combined with a path exits 2', () => {
		const result = run_cli(['format', '--content', 'const x = 1;', '--parser', 'ts', 'a.ts']);
		assert.equal(result.status, 2);
		assert.match(result.stderr, /cannot be combined with file paths/);
	});

	it('format --parser with paths exits 2 (paths use extension detection)', () => {
		const result = run_cli(['format', '--parser', 'ts', 'a.ts']);
		assert.equal(result.status, 2);
		assert.match(result.stderr, /applies to --content/);
	});

	it('format --jobs is accepted in path mode and ignored', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'a.ts'), 'const x = 1;\n');
			const result = run_cli(['format', '--check', '--jobs', '4', dir]);
			assert.equal(result.status, 0);
			assert.match(result.stderr, /0 would change, 1 unchanged/);
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('format --jobs with --content exits 2 (path mode only)', () => {
		const result = run_cli([
			'format',
			'--jobs',
			'4',
			'--content',
			'const x = 1;',
			'--parser',
			'ts'
		]);
		assert.equal(result.status, 2);
		assert.match(result.stderr, /--jobs applies to file paths/);
	});

	it('format --jobs with a non-integer value exits 1 (argument-parsing error)', () => {
		const result = run_cli(['format', '--jobs', 'many', 'a.ts']);
		assert.equal(result.status, 1);
		assert.match(result.stderr, /--jobs expects an integer/);
	});

	it('format on a trailing-slash root reports single-slash paths (PathBuf::push parity)', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'a.ts'), 'const   x=1');
			const result = run_cli(['format', '--check', `${dir}/`]);
			assert.equal(result.status, 1);
			assert.equal(result.stdout, `${dir}/a.ts\n`);
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('format dedupes overlapping root spellings by canonical path', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'a.ts'), 'const   x=1');
			// the same file via a relative traversal ('.') and an absolute explicit arg
			const result = run_cli(['format', '.', join(dir, 'a.ts')], undefined, dir);
			assert.equal(result.status, 0);
			assert.match(result.stderr, /1 formatted, 0 unchanged/);
			assert.equal(result.stdout.trim().split('\n').length, 1);
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	// An explicit file arg is trusted past the *ignore files*, not past the
	// extension check — the two reach the same "not formatted" answer by different
	// routes: traversal filters the file out of scope, the explicit arg is an
	// argument error.
	it('format trusts an explicit file arg past the ignore files, not past the extension', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'a.txt'), 'const   x=1');
			// traversal skips it (no supported extension)…
			const traversed = run_cli(['format', dir]);
			assert.equal(traversed.status, 2);
			assert.match(traversed.stderr, /No files to format/);
			// …and naming it is an argument error, not a TypeScript parse
			const explicit = run_cli(['format', join(dir, 'a.txt')]);
			assert.equal(explicit.status, 2);
			assert.match(explicit.stderr, /unsupported file extension/);
			assert.equal(readFileSync(join(dir, 'a.txt'), 'utf-8'), 'const   x=1');

			// the ignore files, though, really are bypassed by an explicit arg
			writeFileSync(join(dir, '.formatignore'), 'b.ts\n');
			writeFileSync(join(dir, 'b.ts'), 'const   x=1');
			const ignored = run_cli(['format', join(dir, 'b.ts')]);
			assert.equal(ignored.status, 0);
			assert.equal(readFileSync(join(dir, 'b.ts'), 'utf-8'), 'const x = 1;\n');
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('parse --content prints compact JSON with trailing newline', () => {
		const result = run_cli(['parse', '--content', 'const x = 1;', '--parser', 'typescript']);
		assert.equal(result.status, 0);
		assert.ok(result.stdout.endsWith('\n'));
		assert.equal(JSON.parse(result.stdout).type, 'Program');
	});

	it('parse --pretty prints tab-indented JSON', () => {
		const result = run_cli(['parse', '--pretty', '--content', 'const x = 1;', '--parser', 'ts']);
		assert.equal(result.status, 0);
		assert.match(result.stdout, /^\{\n\t"type": "Program",\n/);
	});

	it('parse a file detects the parser from the extension', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'a.svelte'), '<div>x</div>');
			const result = run_cli(['parse', join(dir, 'a.svelte')]);
			assert.equal(result.status, 0);
			assert.equal(JSON.parse(result.stdout).type, 'Root');
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('parse --parser overrides the file extension', () => {
		const dir = mkdtempSync(join(tmpdir(), 'tsv-cli-test-'));
		try {
			writeFileSync(join(dir, 'data.ts'), '<div>x</div>');
			const result = run_cli(['parse', '--parser', 'svelte', join(dir, 'data.ts')]);
			assert.equal(result.status, 0);
			assert.equal(JSON.parse(result.stdout).type, 'Root');
		} finally {
			rmSync(dir, { recursive: true, force: true });
		}
	});

	it('parse on invalid syntax exits 1', () => {
		const result = run_cli(['parse', '--content', 'const =', '--parser', 'ts']);
		assert.equal(result.status, 1);
		assert.match(result.stderr, /Parse error/);
	});

	it('parse rejects a second positional', () => {
		const result = run_cli(['parse', 'a.ts', 'b.ts']);
		assert.equal(result.status, 1);
		assert.match(result.stderr, /Unrecognized argument/);
	});

	it('no command prints usage and exits 1', () => {
		const result = run_cli([]);
		assert.equal(result.status, 1);
		assert.match(result.stderr, /Usage: tsv/);
	});

	it('--help exits 0', () => {
		const result = run_cli(['--help']);
		assert.equal(result.status, 0);
		assert.match(result.stdout, /Usage: tsv/);
	});

	it('--version prints the package version in the native shape', () => {
		const result = run_cli(['--version']);
		assert.equal(result.status, 0, result.stderr);
		const pkg = JSON.parse(
			readFileSync(new URL(`../${pkg_dir}/package.json`, import.meta.url), 'utf-8')
		);
		assert.equal(result.stdout, `tsv ${pkg.version}\n`);
	});

	it('help subcommand exits 0 (mirrors argh)', () => {
		const result = run_cli(['help', 'format']);
		assert.equal(result.status, 0);
		assert.match(result.stdout, /Usage: tsv format/);
	});
});

// Discovery parity — the wasm-package consumer of the shared scenario table
// (see `scripts/discovery_parity_suite.ts`; `scripts/test_napi_npm.ts` runs the
// same suite over the native package's cli.js). The `all` variant is the only
// one shipping cli.js.
register_discovery_parity_suite(
	`discovery parity (cli.js): ${pkg_dir}`,
	new URL(`../${pkg_dir}/cli.js`, import.meta.url).pathname,
	{ skip: variant !== 'all' }
);
