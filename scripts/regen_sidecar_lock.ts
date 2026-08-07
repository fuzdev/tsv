/**
 * Regenerate the Deno sidecar's lockfile (`crates/tsv_debug/src/deno/deno.lock`).
 *
 * The sidecar resolves its npm tree against that lockfile FROZEN (see
 * `crates/tsv_debug/src/deno/actor.rs` `SIDECAR_LOCK`), which is what pins the
 * oracle's own transitive dependencies — above all `esrap`, the printer that
 * emits the JS `svelte.compile()` returns and therefore the effective oracle for
 * every compile fixture. Nothing else in the repo pins it: svelte depends on
 * `esrap@^2.2.12`, a caret.
 *
 * Because the lock is frozen at runtime it can only be rewritten deliberately,
 * here. Run this after bumping any canonical pin in `sidecar.ts`, then:
 *
 *   1. diff the lock — a canonical bump should move the bumped package, and any
 *      OTHER package that moved with it is the oracle shifting under you;
 *   2. update `LOCKED_TRANSITIVE` in `scripts/check_canonical_pins.ts` if a
 *      pinned transitive version changed;
 *   3. re-run `deno task compile:fixtures:validate` (oracle freshness) and
 *      `deno task pins:audit`.
 *
 * Generation happens in a scratch directory rather than in place: `deno cache`
 * writes the lock next to its config, and a stray `deno.json`/`deno.lock` inside
 * `crates/` would be picked up as workspace config by unrelated deno
 * invocations. That scratch lives under `target/` rather than the system temp
 * dir so the task's write permission can be named exactly (`target` +
 * the lockfile's own directory) instead of granted wholesale.
 *
 * The acorn import map is READ OUT of `actor.rs` rather than restated, so the
 * lock is always generated under the same resolution the sidecar actually runs.
 */

const ACTOR_PATH = 'crates/tsv_debug/src/deno/actor.rs';
const LOCK_PATH = 'crates/tsv_debug/src/deno/deno.lock';
const SIDECAR_PATH = 'crates/tsv_debug/src/deno/sidecar.ts';

const check = Deno.args.includes('--check');

/**
 * Resolve versions published within deno's `minimumDependencyAge` window
 * (default 24h) instead of falling back to the newest package old enough.
 *
 * That window is a supply-chain guard — it exists so a compromised release
 * cannot be pulled in the hour it lands — so this is deliberately OPT-IN rather
 * than the script's default. Reach for it only when a just-published upstream
 * version is one we've decided to take now rather than tomorrow; without it,
 * deno silently resolves the previous version and the lock looks stale for a
 * reason nothing reports.
 *
 * A lock produced with this flag is NOT flag-dependent: once the version ages
 * past the window, a plain `deno task pins:lock` reproduces it byte-for-byte.
 */
const allow_fresh = Deno.args.includes('--allow-fresh');

const actor = Deno.readTextFileSync(ACTOR_PATH);
const acorn_pin = /npm:acorn@(\d+\.\d+\.\d+)/.exec(actor)?.[1];
if (!acorn_pin) {
	console.error(`FAIL: no npm:acorn@x.y.z import-map pin found in ${ACTOR_PATH}`);
	Deno.exit(1);
}

const SCRATCH_ROOT = 'target';

/**
 * Generate the lock and reconcile it with the committed one, returning an exit
 * code.
 *
 * Returns rather than calling `Deno.exit` because `Deno.exit` does NOT unwind —
 * it skips `finally`, so exiting from inside the scratch bracket leaks the
 * directory (which is how the first version of this script left `target/`
 * littered on every failure).
 */
const regenerate = (dir: string): number => {
	Deno.copyFileSync(SIDECAR_PATH, `${dir}/sidecar.ts`);
	Deno.writeTextFileSync(
		`${dir}/deno.json`,
		JSON.stringify({
			imports: { acorn: `npm:acorn@${acorn_pin}` },
			lock: './deno.lock',
			...(allow_fresh ? { minimumDependencyAge: '0' } : {})
		})
	);

	// `deno cache` resolves every import and writes the lock WITHOUT running the
	// sidecar (which would block reading its stdin protocol). No `cwd` override:
	// these paths are relative to the repo root, and `lock: './deno.lock'` is
	// resolved against the CONFIG's directory regardless of cwd.
	const { success } = new Deno.Command('deno', {
		args: ['cache', '--config', `${dir}/deno.json`, `${dir}/sidecar.ts`],
		stdout: 'inherit',
		stderr: 'inherit'
	}).outputSync();
	if (!success) {
		console.error('FAIL: `deno cache` could not resolve the sidecar imports');
		return 1;
	}

	const generated = Deno.readTextFileSync(`${dir}/deno.lock`);
	let committed: string | null;
	try {
		committed = Deno.readTextFileSync(LOCK_PATH);
	} catch {
		committed = null;
	}

	if (generated === committed) {
		console.log(`sidecar lock already up to date (${LOCK_PATH})`);
		return 0;
	}
	if (check) {
		// Deliberately NOT wired into a gate: resolution depends on what the
		// registry currently offers, so this would fail on a newly published
		// upstream version — which is drift to decide about, not a red build.
		console.error(
			`FAIL: ${LOCK_PATH} is not what the current registry resolves — ` +
				'run `deno task pins:lock` and review the diff'
		);
		return 1;
	}
	Deno.writeTextFileSync(LOCK_PATH, generated);
	console.log(
		`wrote ${LOCK_PATH} — diff it, then re-run \`deno task pins:audit\` and ` +
			'`deno task compile:fixtures:validate`'
	);
	return 0;
};

Deno.mkdirSync(SCRATCH_ROOT, { recursive: true });
const dir = Deno.makeTempDirSync({ dir: SCRATCH_ROOT, prefix: 'sidecar_lock_' });
let code: number;
try {
	code = regenerate(dir);
} finally {
	Deno.removeSync(dir, { recursive: true });
}
Deno.exit(code);
