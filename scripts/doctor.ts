/**
 * `deno task doctor` — one-pass, read-only setup check for tsv's diagnostic
 * toolchain. The per-tool preflights fail loud at use time; this answers "am I
 * set up?" BEFORE a run: runtimes, the canonical-oracle pin agreement, the
 * benches/js `node_modules` (installed + fresh), the sibling oracle checkouts
 * (presence + version skew vs the npm pins), the corpus entry lists, and which
 * build artifacts exist.
 *
 * Exit semantics: exit 1 only for states that would MISLEAD a run or break the
 * core toolchain (canonical pin drift, checkout↔pin skew, stale node_modules,
 * missing cargo) — mere absences (a checkout not cloned, an artifact not
 * built) are ⚠ warnings: they fail loud on their own at use time, and plenty
 * of workflows don't need them. So a green doctor means "nothing on this
 * machine will lie to you", not "everything is installed". `--strict` promotes
 * warnings to failures — the "is this machine FULLY provisioned?" mode for a
 * release box.
 *
 * Requires --config benches/js/deno.json (the corpus probe resolves the
 * harness's npm deps via nodeModulesDir: manual).
 */

import { probe_node_modules } from '../benches/js/lib/check_node_modules.ts';
import { native_library_filename } from '../benches/js/lib/runtime.ts';
import { load_all_versions } from '../benches/js/lib/versions.ts';

let warnings = 0;
let errors = 0;

function ok(text: string): void {
	console.log(`  ✓ ${text}`);
}
function warn(text: string): void {
	warnings++;
	console.log(`  ⚠ ${text}`);
}
function fail(text: string): void {
	errors++;
	console.log(`  ✗ ${text}`);
}
function info(text: string): void {
	console.log(`  · ${text}`);
}

function section(title: string): void {
	console.log(`\n${title}`);
}

/** First line of `cmd --version`, or null when the binary is missing/failing. */
function run_version(cmd: string, args: string[] = ['--version']): string | null {
	try {
		const out = new Deno.Command(cmd, { args, stdout: 'piped', stderr: 'piped' }).outputSync();
		if (!out.success) return null;
		return new TextDecoder().decode(out.stdout).trim().split('\n')[0];
	} catch {
		return null;
	}
}

function exists(path: string): boolean {
	try {
		Deno.statSync(path);
		return true;
	} catch {
		return false;
	}
}

function read_pkg_version(path: string): string | null {
	try {
		const pkg = JSON.parse(Deno.readTextFileSync(path)) as { version?: string };
		return pkg.version ?? null;
	} catch {
		return null;
	}
}

const strict = Deno.args.includes('--strict');

console.log(
	`tsv doctor — diagnostic-toolchain setup check (read-only${strict ? ', --strict: warnings fail' : ''})`,
);

// --- Runtimes -----------------------------------------------------------------

section('Runtimes');
ok(`deno ${Deno.version.deno}`);

const node_version = run_version('node');
if (node_version === null) {
	warn('node missing — bench:node, bench:install, test:npm, and the publish artifact tests need it');
} else {
	const m = /^v(\d+)\.(\d+)/.exec(node_version);
	const new_enough = m !== null && (Number(m[1]) > 22 || (Number(m[1]) === 22 && Number(m[2]) >= 18));
	if (new_enough) ok(`node ${node_version} (≥ 22.18)`);
	else warn(`node ${node_version} — < 22.18 lacks native TS type-stripping; the harness entries fail to parse`);
}

const bun_version = run_version('bun');
if (bun_version === null) warn('bun missing — bench:bun unavailable (optional)');
else ok(`bun ${bun_version}`);

const cargo_version = run_version('cargo');
if (cargo_version === null) fail('cargo missing — nothing Rust builds without it');
else ok(cargo_version);

const wasm_pack_version = run_version('wasm-pack');
if (wasm_pack_version === null) {
	warn('wasm-pack missing — WASM builds (build:wasm:*, bench, publish) unavailable: cargo install wasm-pack');
} else ok(wasm_pack_version);

const npm_version = run_version('npm');
if (npm_version === null) warn('npm missing — deno task bench:install unavailable');
else ok(`npm ${npm_version}`);

// --- Canonical pins -----------------------------------------------------------

section('Canonical oracle pins');
const pins = new Deno.Command('deno', {
	args: ['run', '--allow-read', 'scripts/check_canonical_pins.ts'],
	stdout: 'piped',
	stderr: 'piped',
}).outputSync();
const pins_out = new TextDecoder().decode(pins.success ? pins.stdout : pins.stderr).trim();
if (pins.success) ok(pins_out);
else fail(pins_out.split('\n').join('\n    '));

// --- Harness deps ---------------------------------------------------------------

section('Harness deps (benches/js/node_modules)');
const nm = await probe_node_modules();
if (nm.status === 'ok') ok('installed + fresh (npm install stamp is newer than package.json)');
else if (nm.status === 'missing') warn(nm.message);
else fail(`${nm.message} — reports would label OLD installed versions with the new pins`);

// --- Oracle checkouts -----------------------------------------------------------

section('Oracle checkouts (conformance gates / publish Step 3b)');
const versions = await load_all_versions();

if (exists('../svelte/packages/svelte/tests')) {
	const v = read_pkg_version('../svelte/packages/svelte/package.json');
	if (v !== null && v !== versions.canonical.svelte) {
		warn(
			`../svelte checkout is v${v} but the svelte oracle is pinned v${versions.canonical.svelte} — ` +
				'suite inputs and the grading parser disagree (align the checkout or bump the pins deliberately)',
		);
	} else ok(`../svelte checkout (v${v ?? '?'}, matches the pin)`);
} else warn('../svelte checkout missing — conformance:svelte-fixtures + the corpus suites need it');

if (exists('../acorn-typescript/test')) {
	const v = read_pkg_version('../acorn-typescript/package.json');
	if (v !== null && v !== versions.canonical['@sveltejs/acorn-typescript']) {
		warn(
			`../acorn-typescript checkout is v${v} but the oracle is pinned ` +
				`v${versions.canonical['@sveltejs/acorn-typescript']} — suite inputs and the grading parser disagree`,
		);
	} else ok(`../acorn-typescript checkout (v${v ?? '?'}, matches the pin)`);
} else warn('../acorn-typescript checkout missing — conformance:ts-fixtures needs it');

if (exists('../typescript/tests/baselines/reference')) {
	ok('../typescript checkout (baselines present)');
} else if (exists('../typescript')) {
	warn('../typescript present but tests/baselines/reference missing — conformance:ts-repo will FAIL (partial checkout)');
} else warn('../typescript checkout missing — conformance:ts-repo needs it');

// Git-SHA-pinned like ../typescript (not npm-versioned), so — same as ../typescript
// — it isn't in pins:audit; presence is checked here, and its pinned tsgo commit is
// enforced by the Rust roundtrip count-pins (ROUNDTRIP_PASS_PIN / BASELINE_COUNT_PIN).
if (exists('../typescript-go/testdata/baselines/reference/submodule')) {
	ok('../typescript-go checkout (tsgo baselines present)');
} else if (exists('../typescript-go')) {
	warn('../typescript-go present but testdata/baselines/reference/submodule missing — conformance:tsc-roundtrip will FAIL (partial checkout)');
} else warn('../typescript-go checkout missing — conformance:tsc-roundtrip needs it');

// The tsc-check leg additionally sweeps the corpus INPUTS + bundled libs (unlike
// roundtrip, which reads only the committed baselines). The corpus is the
// often-unmaterialized _submodules/TypeScript submodule.
if (exists('../typescript-go')) {
	if (exists('../typescript-go/_submodules/TypeScript/tests/cases')) {
		ok('../typescript-go corpus inputs (_submodules/TypeScript materialized)');
	} else {
		warn('../typescript-go corpus inputs missing — conformance:tsc-check needs them (git submodule update --init in ../typescript-go)');
	}
	if (exists('../typescript-go/internal/bundled/libs')) {
		ok('../typescript-go bundled libs present');
	} else {
		warn('../typescript-go bundled libs (internal/bundled/libs) missing — conformance:tsc-check needs them');
	}
}

// Informational (NOT gated by pins:audit — see its docstring): the prettier
// checkout is a reading reference + corpus-suite source whose oracle output is
// computed live per file, and it legitimately rides `-dev` versions.
if (exists('../prettier')) {
	const v = read_pkg_version('../prettier/package.json');
	if (v !== null && v.replace(/-dev$/, '') !== versions.canonical.prettier) {
		warn(
			`../prettier checkout is v${v} vs pinned prettier v${versions.canonical.prettier} — its fixture ` +
				'suites (corpus inputs) come from a different version than the live oracle (informational)',
		);
	} else ok(`../prettier checkout (v${v ?? '?'})`);
} else warn('../prettier checkout missing — the corpus prettier suites + layout-reference reading need it');

if (exists('../test262/test')) ok('../test262 checkout');
else warn('../test262 checkout missing — tsv_debug test262 + bench:harvest:test262 unavailable (manual-cadence tools)');

if (exists('../wpt/css')) ok('../wpt/css checkout (sparse)');
else warn('../wpt/css checkout missing — bench:harvest:wpt unavailable (manual-cadence tool)');

// --- Corpus entries -------------------------------------------------------------

section('Corpus entries (gates + conformance views)');
try {
	const { corpus_missing_entries } = await import('../benches/js/lib/corpus.ts');
	for (const view of ['gates', 'conformance'] as const) {
		const { missing, optional_missing, total } = await corpus_missing_entries(view);
		if (missing.length > 0) {
			warn(
				`${view} view: ${missing.length}/${total} entries missing — ` +
					`corpus:compare/bench on this view fail fast:\n      ${missing.join('\n      ')}`,
			);
		} else ok(`${view} view: all ${total - optional_missing.length}/${total} required entries present`);
		for (const o of optional_missing) info(`${view} view: optional entry absent (fail-open, disclosed): ${o}`);
	}
} catch (e) {
	warn(
		`cannot load the corpus entry list (${e instanceof Error ? e.message.split('\n')[0] : e}) — ` +
			'usually node_modules missing; run deno task bench:install',
	);
}

// --- Build artifacts (informational) --------------------------------------------

section('Build artifacts (built on demand — absence is normal before a run)');
const ffi_lib = native_library_filename('tsv_ffi');
const napi_lib = native_library_filename('tsv_napi');
const artifacts: [string, string][] = [
	[`target/release/${ffi_lib}`, 'deno task build:ffi'],
	[`target/corpus/${ffi_lib}`, 'deno task build:ffi:corpus (conformance gates)'],
	[`target/release/${napi_lib}`, 'deno task build:napi (bench:node/bun)'],
	['crates/tsv_wasm/pkg/all/deno', 'deno task build:wasm:all:deno (bench:deno)'],
	['crates/tsv_wasm/pkg/all/nodejs', 'deno task build:wasm:all:nodejs (bench:node/bun)'],
	['crates/tsv_wasm/pkg/all/npm', 'deno task build:npm:all (publish)'],
];
for (const [path, task] of artifacts) {
	if (exists(path)) ok(path);
	else info(`${path} absent — ${task}`);
}
info('freshness (artifact vs source mtimes) is enforced at run time by lib/check_artifact_freshness.ts');
info('Deno sidecar (fixtures/compare tooling): verify with `cargo run -p tsv_debug check`');

// --- Verdict --------------------------------------------------------------------

console.log('');
if (errors > 0) {
	console.log(`✗ doctor: ${errors} error(s), ${warnings} warning(s) — the ✗ items above would mislead a run; fix them first.`);
	Deno.exit(1);
}
if (warnings > 0) {
	if (strict) {
		console.log(`✗ doctor --strict: ${warnings} warning(s) — this machine is not fully provisioned.`);
		Deno.exit(1);
	}
	console.log(`⚠ doctor: ${warnings} warning(s) — nothing misleading; the ⚠ tools fail loud on their own if used.`);
} else {
	console.log('✓ doctor: everything present and consistent.');
}
