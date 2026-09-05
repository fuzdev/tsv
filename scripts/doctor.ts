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
 * release box. One exception: in the explicitly optional
 * experimental-typechecker tier an ABSENCE reports as · info, never ⚠, so it
 * stays inert even under `--strict` — that oracle serves no release and no
 * ordinary dev flow, so a fully-provisioned release box legitimately lacks it.
 * A BROKEN checkout there (present, but its baselines missing) still warns:
 * that is the misleading state this tool exists to catch.
 *
 * Requires --config benches/js/deno.json (the corpus probe resolves the
 * harness's npm deps via nodeModulesDir: manual).
 */

import { probe_node_modules } from '../benches/js/lib/check_node_modules.ts';
import { CORPORA_ROOT, CORPORA_TREE, CORPORA_URL } from '../benches/js/lib/corpora.ts';
import { GATE_CHECKOUT_IDS } from '../benches/js/lib/gate_counts.ts';
import {
	checkout_hash,
	checkout_label,
	HARVEST_STAMPS,
	short_commit
} from '../benches/js/lib/harvest_stamp.ts';
import { native_library_filename } from '../benches/js/lib/runtime.ts';
import { type AllVersions, load_all_versions } from '../benches/js/lib/versions.ts';

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

/** Whether `git status` reports any change under `subpath` of the checkout at `repo`. */
function git_dirty(repo: string, subpath: string): boolean {
	try {
		const out = new Deno.Command('git', {
			args: ['-C', repo, 'status', '--porcelain', '--', subpath],
			stdout: 'piped',
			stderr: 'null'
		}).outputSync();
		return out.success && out.stdout.length > 0;
	} catch {
		return false;
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

/**
 * The distinct versions `check.yml` installs for `tool` — one entry when its jobs
 * agree, empty when the workflow can't be read.
 *
 * READ from the workflow rather than mirrored into a constant here, unlike
 * `WASM_PACK_PIN` below: that one pins a calibration this repo owns (the wasm size
 * bounds), so a local constant IS its source of truth, while deno and node are
 * pinned in CI and nowhere else — a mirror would just be a second place to forget.
 * The drift this catches is one-directional and quiet: an upgraded local runtime
 * leaves CI on the old one, every gate stays green on both, and the machine that
 * produced the committed bench reports is no longer the machine CI verifies.
 */
function ci_pin(tool: 'deno' | 'node'): ReadonlyArray<string> {
	try {
		const yml = Deno.readTextFileSync('.github/workflows/check.yml');
		// `node-version: '24.14.1'` is quoted, `deno-version: 2.9.5` is not. EVERY
		// occurrence, not the first: the workflow sets each pin once per job, so a
		// bump that reached one job and not the others is its own drift — and reading
		// only the first would report the machine aligned while a job trails.
		const all = [...yml.matchAll(new RegExp(`${tool}-version:\\s*'?([\\d.]+)'?`, 'g'))];
		return [...new Set(all.map((m) => m[1]))];
	} catch {
		return [];
	}
}

/** Warn when the installed version differs from what CI pins — see `ci_pin`. */
function check_ci_pin(tool: 'deno' | 'node', installed: string): void {
	const pins = ci_pin(tool);
	if (pins.length === 0) return;
	if (pins.length > 1) {
		warn(
			`${tool} — .github/workflows/check.yml pins more than one version (${pins.join(', ')}); ` +
				`its jobs disagree about which ${tool} verifies this repo`
		);
		return;
	}
	// Exact, not `includes` as the wasm-pack check below does: both versions here are
	// already bare (`2.9.5`, and node's `v` stripped by the caller), so a substring
	// test would only buy the false pass where `2.9.5` matches an installed `2.9.50`.
	if (installed === pins[0]) return;
	warn(
		`${tool} ${installed} — differs from the ${pins[0]} .github/workflows/check.yml installs; ` +
			`bump the workflow (${tool}-version) so CI verifies what you develop against`
	);
}

const strict = Deno.args.includes('--strict');

console.log(
	`tsv doctor — diagnostic-toolchain setup check (read-only${strict ? ', --strict: warnings fail' : ''})`
);

// --- Runtimes -----------------------------------------------------------------

section('Runtimes');
ok(`deno ${Deno.version.deno}`);
check_ci_pin('deno', Deno.version.deno);

const node_version = run_version('node');
if (node_version === null) {
	warn(
		'node missing — bench:node, bench:install, test:npm, and the publish artifact tests need it'
	);
} else {
	const m = /^v(\d+)\.(\d+)/.exec(node_version);
	const new_enough =
		m !== null && (Number(m[1]) > 22 || (Number(m[1]) === 22 && Number(m[2]) >= 18));
	if (new_enough) ok(`node ${node_version} (≥ 22.18)`);
	else warn(
		`node ${node_version} — < 22.18 lacks native TS type-stripping; the harness entries fail to parse`
	);
	check_ci_pin('node', node_version.replace(/^v/, ''));
}

const bun_version = run_version('bun');
if (bun_version === null) warn('bun missing — bench:bun unavailable (optional)');
else ok(`bun ${bun_version}`);

const cargo_version = run_version('cargo');
if (cargo_version === null) fail('cargo missing — nothing Rust builds without it');
else ok(cargo_version);

// The one version the repo otherwise records only in CI (`check.yml` pins the
// same): the published wasm sizes are produced by wasm-pack's bundled wasm-opt,
// and `validate_artifacts.ts`'s ±8% bounds are calibrated against this version —
// a different wasm-pack can move the bytes into failing bounds at publish time.
const WASM_PACK_PIN = '0.15.0';
const wasm_pack_version = run_version('wasm-pack');
if (wasm_pack_version === null) {
	warn(
		'wasm-pack missing — WASM builds (build:wasm:*, bench, publish) unavailable: cargo install wasm-pack'
	);
} else if (!wasm_pack_version.includes(WASM_PACK_PIN)) {
	warn(
		`${wasm_pack_version} — differs from the pinned ${WASM_PACK_PIN} (check.yml + the ` +
			`validate_artifacts.ts size-bound calibration); sizes may drift into failing bounds at publish`
	);
} else ok(`${wasm_pack_version} (matches pin)`);

const npm_version = run_version('npm');
if (npm_version === null) warn('npm missing — deno task bench:install unavailable');
else ok(`npm ${npm_version}`);

// --- Canonical pins -----------------------------------------------------------

section('Canonical oracle pins');
// No mode flag = BOTH halves — the pin agreement `deno task check` gates plus the
// checkout alignment/drift `deno task conformance` gates. Doctor is the one place
// that reports them together, which is what makes an env skew visible before a
// conformance run rather than during one. `--allow-run=git` is load-bearing: the
// commit-drift half shells out to `git rev-parse`, and without it every checkout
// reads as absent.
const pins = new Deno.Command('deno', {
	args: ['run', '--allow-read', '--allow-run=git', 'scripts/check_canonical_pins.ts'],
	stdout: 'piped',
	stderr: 'piped'
}).outputSync();
const pins_out = new TextDecoder().decode(pins.success ? pins.stdout : pins.stderr).trim();
if (pins.success) ok(pins_out);
else fail(pins_out.split('\n').join('\n    '));

// --- Harness deps ---------------------------------------------------------------

section('Harness deps (benches/js/node_modules)');
// Both this probe and `load_all_versions` below THROW when `benches/js/package.json`
// can't be read or parsed — the right posture for a run that would otherwise measure
// under placeholder labels, and the wrong one HERE: doctor's whole job is to report a
// broken setup, so a broken pins file must be a `✗` line, not an unhandled rejection
// that kills the remaining sections and the verdict. Same guard as the corpus probe.
try {
	const nm = await probe_node_modules();
	if (nm.status === 'ok') {
		ok('installed, and every exact pin (plus the oxc wasi binding) matches the installed version');
	} else if (nm.status === 'missing') warn(nm.message);
	else fail(`${nm.message} — reports would label OLD installed versions with the new pins`);
} catch (e) {
	fail(
		`cannot read benches/js/package.json (${e instanceof Error ? e.message.split('\n')[0] : e}) — ` +
			'the pins are unreadable, so nothing downstream can be graded against them'
	);
}

// --- Oracle checkouts -----------------------------------------------------------

section('Oracle checkouts (conformance gates / publish Step 3b)');
// Guarded for the same reason as the probe above: this throws on an unreadable pins
// file, which is a state to REPORT here rather than to die on. `null` then — the
// checkouts are still worth listing, but no skew can be graded against a pin nothing
// could read, so the per-checkout lines say that instead of claiming a match.
let versions: AllVersions | null = null;
try {
	versions = await load_all_versions();
} catch (e) {
	fail(
		`cannot read the oracle pins (${e instanceof Error ? e.message.split('\n')[0] : e}) — ` +
			'checkout-vs-pin skew is ungraded below'
	);
}
const pin_suffix = versions ? ', matches the pin' : '; pin unreadable';

if (exists('../svelte/packages/svelte/tests')) {
	const v = read_pkg_version('../svelte/packages/svelte/package.json');
	if (versions && v !== null && v !== versions.canonical.svelte) {
		warn(
			`../svelte checkout is v${v} but the svelte oracle is pinned v${versions.canonical.svelte} — ` +
				'suite inputs and the grading parser disagree (align the checkout or bump the pins deliberately)'
		);
	} else ok(`../svelte checkout (v${v ?? '?'}${pin_suffix})`);
} else warn('../svelte checkout missing — conformance:svelte-fixtures + the corpus suites need it');

if (exists('../acorn-typescript/test')) {
	const v = read_pkg_version('../acorn-typescript/package.json');
	if (versions && v !== null && v !== versions.canonical['@sveltejs/acorn-typescript']) {
		warn(
			`../acorn-typescript checkout is v${v} but the oracle is pinned ` +
				`v${versions.canonical['@sveltejs/acorn-typescript']} — suite inputs and the grading parser disagree`
		);
	} else ok(`../acorn-typescript checkout (v${v ?? '?'}${pin_suffix})`);
} else warn('../acorn-typescript checkout missing — conformance:ts-fixtures needs it');

if (exists('../typescript/tests/baselines/reference')) {
	ok('../typescript checkout (baselines present)');
} else if (exists('../typescript')) {
	warn(
		'../typescript present but tests/baselines/reference missing — conformance:ts-repo will FAIL (partial checkout)'
	);
} else warn('../typescript checkout missing — conformance:ts-repo + bench:harvest:ts-repo need it');

// Informational (NOT gated by pins:audit — see its docstring): the prettier
// checkout is a reading reference + corpus-suite source whose oracle output is
// computed live per file, and it legitimately rides `-dev` versions.
if (exists('../prettier')) {
	const v = read_pkg_version('../prettier/package.json');
	if (versions && v !== null && v.replace(/-dev$/, '') !== versions.canonical.prettier) {
		warn(
			`../prettier checkout is v${v} vs pinned prettier v${versions.canonical.prettier} — its fixture ` +
				'suites (corpus inputs) come from a different version than the live oracle (informational)'
		);
	} else ok(`../prettier checkout (v${v ?? '?'})`);
} else warn(
	'../prettier checkout missing — the corpus prettier suites + layout-reference reading need it'
);

if (exists('../test262/test')) ok('../test262 checkout (conformance:test262 release gate)');
else warn(
	'../test262 checkout missing — conformance:test262 (the release gate, publish Step 3b) + bench:harvest:test262 need it'
);

if (exists('../wpt/css')) ok('../wpt/css checkout (sparse)');
else warn('../wpt/css checkout missing — bench:harvest:wpt unavailable (manual-cadence tool)');

// The real-code corpus snapshot: every `real`/`framework` corpus entry reads one of
// its collections, and the corpus count pins were measured at the `collections/` tree
// id `GATE_CHECKOUT_IDS` records — so a checkout whose collections are at any
// other id grades a different corpus than the pins describe (a refresh re-pins;
// anything else is skew). The tree, not the commit: the snapshot repo's tooling
// commits move HEAD without moving a corpus byte.
const corpora_pin = GATE_CHECKOUT_IDS[CORPORA_ROOT].hash;
const corpora_head = checkout_hash({ path: CORPORA_ROOT, tree: CORPORA_TREE });
if (corpora_head === null) {
	warn(
		`${CORPORA_ROOT} checkout missing — the bench + corpus:compare gates read the real-code snapshot ` +
			`(git clone ${CORPORA_URL} ${CORPORA_ROOT})`
	);
} else if (!corpora_head.startsWith(corpora_pin)) {
	warn(
		`${CORPORA_ROOT}:${CORPORA_TREE} is at ${short_commit(corpora_head)} but the corpus pins were measured at ${corpora_pin} — ` +
			'a snapshot refresh re-runs the corpus gates and re-pins; otherwise check out the pinned snapshot'
	);
} else if (git_dirty(CORPORA_ROOT, CORPORA_TREE)) {
	// The gates read the working tree, so bytes the pinned commit doesn't hold would
	// be graded under its name. Nothing in tsv writes there; a stray `tsv format` on a
	// collection is the way this happens.
	warn(
		`${CORPORA_ROOT} is at pinned ${corpora_pin} but ${CORPORA_TREE}/ has local modifications — ` +
			`the corpus gates would grade bytes that commit does not hold (git -C ${CORPORA_ROOT} checkout -- ${CORPORA_TREE})`
	);
} else ok(`${CORPORA_ROOT} snapshot (${CORPORA_TREE}/ at pinned ${corpora_pin}, clean)`);

// --- Optional: the experimental typechecker's oracle -----------------------------
//
// `../typescript-go` feeds ONLY the on-demand `conformance:tsc-roundtrip` /
// `conformance:tsc-check` tasks for the experimental `tsv_check` crate — not
// `deno task check`, not the release gates. A machine that never touches the
// typechecker needs none of it, so every finding here is a warning at most.
// Git-SHA-pinned (not npm-versioned), so it isn't in pins:audit either — its tsgo
// commit is pinned by the Rust count-pins. See docs/typechecker.md.

section('Optional — experimental typechecker oracle (on-demand tsc_conformance only)');

if (exists('../typescript-go/testdata/baselines/reference/submodule')) {
	ok('../typescript-go checkout (tsgo baselines present)');
} else if (exists('../typescript-go')) {
	warn(
		'../typescript-go present but testdata/baselines/reference/submodule missing — conformance:tsc-roundtrip would FAIL (partial checkout)'
	);
} else info(
	'../typescript-go checkout absent — needed only by the on-demand tsc_conformance tasks'
);

// The tsc-check task additionally sweeps the corpus INPUTS + bundled libs (unlike
// roundtrip, which reads only the committed baselines). The corpus is the
// often-unmaterialized _submodules/TypeScript submodule.
if (exists('../typescript-go')) {
	if (exists('../typescript-go/_submodules/TypeScript/tests/cases')) {
		ok('../typescript-go corpus inputs (_submodules/TypeScript materialized)');
	} else {
		info(
			'../typescript-go corpus inputs absent — only conformance:tsc-check needs them (git submodule update --init in ../typescript-go)'
		);
	}
	if (exists('../typescript-go/internal/bundled/libs')) {
		ok('../typescript-go bundled libs present');
	} else {
		info(
			'../typescript-go bundled libs (internal/bundled/libs) absent — only conformance:tsc-check needs them'
		);
	}
}

// --- Harvest stamps -------------------------------------------------------------
//
// Each stamped grade records the checkout object(s) it ran against — a HEAD commit,
// or for the corpora snapshot its `collections/` tree id. A stamp whose recorded id
// no longer matches the checkout means the pin it graded describes the PREVIOUS
// corpus: `bench:pins:suites` (a `conformance` preflight) re-derives the suite pins
// and `bench:harvest:svelte-styles` the styles one, but nothing in `check` does, so
// name it here ahead of a release run. Only the checkout inputs are compared — the
// oracle-version and pin inputs are `pins:audit`'s.
//
// Graded PER CHECKOUT, never all-or-nothing on the first absent one. Two reasons,
// both of which the earlier short-circuit got wrong: an absent checkout would hide
// a MOVED sibling in the same stamp (the two reject pins carry three checkouts
// apiece), and this doctor cannot know what the task will do about the absence —
// `css:over-acceptance:pin` reads the wpt CACHE, not `../wpt`, so it grades happily
// with that checkout gone. So report the fact observed here, and leave the task's
// own `--if-present` verdict to the task.

section(
	'Harvest stamps (bench:pins:suites — the pin-freshness preflight — + bench:harvest:svelte-styles)'
);
for (const [name, { path, task, checkouts }] of Object.entries(HARVEST_STAMPS)) {
	const heads = Object.entries(checkouts).map(([key, input]) => ({
		key,
		repo: checkout_label(input),
		head: checkout_hash(input)
	}));
	const absent = heads.filter((h) => h.head === null);
	const gradable = heads.filter((h) => h.head !== null);
	// Named on every line below, so a ✓ never reads as "all three agree" when one
	// of the three was never compared.
	const aside =
		absent.length === 0 ? '' : ` (${absent.map((h) => h.repo).join(', ')} not checked out)`;
	if (gradable.length === 0) {
		info(`${name}: no checkout to grade against${aside}`);
		continue;
	}
	let recorded: Record<string, unknown>;
	try {
		recorded = JSON.parse(Deno.readTextFileSync(path)) as Record<string, unknown>;
	} catch {
		warn(`${name}: never stamped (no ${path}) — run deno task ${task}${aside}`);
		continue;
	}
	// A key the stamp does not carry at all is reported as such rather than as a
	// moved commit: that is what an input ADDED since the stamp was written looks
	// like, and "stamped ?" reads as a corrupt SHA.
	const moved = gradable
		.map((h) => ({ ...h, recorded: recorded[h.key] }))
		.filter((h) => h.recorded !== h.head);
	if (moved.length > 0) {
		warn(
			`${name}: stamp is stale for ${moved
				.map((h) =>
					typeof h.recorded === 'string'
						? `${h.repo} (stamped ${short_commit(h.recorded)}, now ${short_commit(h.head!)})`
						: `${h.repo} (no \`${h.key}\` recorded — the stamp predates that input)`
				)
				.join(', ')} — run deno task ${task}${aside}`
		);
	} else ok(`${name}: stamp matches ${gradable.map((h) => h.repo).join(' + ')}${aside}`);
}

// --- Corpus entries -------------------------------------------------------------

section('Corpus entries (gates + conformance views)');
try {
	const { corpus_missing_entries, corpus_untiered_collections } =
		await import('../benches/js/lib/corpus.ts');
	for (const view of ['gates', 'conformance'] as const) {
		const { missing, optional_missing, total } = await corpus_missing_entries(view);
		if (missing.length > 0) {
			warn(
				`${view} view: ${missing.length}/${total} entries missing — ` +
					`corpus:compare/bench on this view fail fast:\n      ${missing.join('\n      ')}`
			);
		} else ok(
			`${view} view: all ${total - optional_missing.length}/${total} required entries present`
		);
		for (const o of optional_missing)
			info(`${view} view: optional entry absent (fail-open, disclosed): ${o}`);
	}
	// A collection the snapshot vendors that no tier places is in no bench or gate view
	// — vendored ahead of its triage, read only by the whole-snapshot sweeps. Listed so
	// it is a standing question rather than a forgotten one.
	const untiered = await corpus_untiered_collections();
	if (untiered.length > 0) {
		info(
			`${untiered.length} snapshot collection(s) in no tier (whole-snapshot sweeps only): ` +
				untiered.join(', ')
		);
	}
} catch (e) {
	// The entry list is derived from the snapshot's manifest, so this is also where a
	// checkout whose manifest this reader can't take (or a tier table naming a collection
	// the manifest dropped) surfaces — the message names which; a bare import failure is
	// usually node_modules missing (deno task bench:install).
	warn(`cannot load the corpus entry list: ${e instanceof Error ? e.message.split('\n')[0] : e}`);
}

// --- Build artifacts (informational) --------------------------------------------

section('Build artifacts (built on demand — absence is normal before a run)');
const ffi_lib = native_library_filename('tsv_ffi');
const napi_lib = native_library_filename('tsv_napi');
const artifacts: [string, string][] = [
	[`target/release/${ffi_lib}`, 'deno task build:ffi'],
	[`target/corpus/${ffi_lib}`, 'deno task build:ffi:corpus (conformance gates)'],
	[`target/napi/${napi_lib}`, 'deno task build:napi (bench:node/bun)'],
	['crates/tsv_wasm/pkg/all/deno', 'deno task build:wasm:all:deno (bench:deno)'],
	['crates/tsv_wasm/pkg/all/nodejs', 'deno task build:wasm:all:nodejs (bench:node/bun)'],
	['crates/tsv_wasm/pkg/all/npm', 'deno task build:npm:all (publish)']
];
for (const [path, task] of artifacts) {
	if (exists(path)) ok(path);
	else info(`${path} absent — ${task}`);
}
info(
	'freshness (artifact vs source mtimes) is enforced at run time by lib/check_artifact_freshness.ts' +
		' (bench/corpus) and scripts/check_staged_freshness.ts (the staged npm/napi packages)'
);
info('Deno sidecar (fixtures/compare tooling): verify with `cargo run -p tsv_debug check`');

// --- Verdict --------------------------------------------------------------------

console.log('');
if (errors > 0) {
	console.log(
		`✗ doctor: ${errors} error(s), ${warnings} warning(s) — the ✗ items above would mislead a run; fix them first.`
	);
	Deno.exit(1);
}
if (warnings > 0) {
	if (strict) {
		console.log(
			`✗ doctor --strict: ${warnings} warning(s) — this machine is not fully provisioned.`
		);
		Deno.exit(1);
	}
	console.log(
		`⚠ doctor: ${warnings} warning(s) — nothing misleading; the ⚠ tools fail loud on their own if used.`
	);
} else {
	console.log('✓ doctor: everything present and consistent.');
}
