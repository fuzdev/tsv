/**
 * Publish the staged N-API npm packages — the release workflow's publish step,
 * also runnable locally with `--dry-run`.
 *
 * Reads `crates/tsv_napi/pkg/`: the loader (`pkg/napi/`) plus every platform
 * package its generated `optionalDependencies` name (the loader package.json
 * is the single source of truth for the expected set — no third copy of the
 * triple list here). Refuses to publish a partial set: a missing platform
 * artifact means a matrix job failed, and a loader pointing at unpublished
 * optionalDependencies would break installs.
 *
 * Publish order is platform packages FIRST, the loader LAST, so the loader
 * never goes live before the binaries it resolves. Idempotent like
 * `scripts/publish.ts`: a version already on the registry is skipped
 * (re-running a partially-failed workflow publishes only what's missing).
 * `npm view` errors other than E404 (auth, network) fail loudly rather than
 * being read as "not published".
 *
 * Usage: deno run --allow-read --allow-env --allow-run=npm scripts/publish_napi.ts \
 *          [--dry-run] [--provenance] [--expect-tag v<version>]
 *
 * `--provenance` is passed by the release workflow (needs the workflow's
 * `id-token: write`); `--expect-tag` asserts the staged version matches the
 * triggering tag before anything publishes.
 */

import { parseArgs } from 'node:util';

const { values: args } = parseArgs({
	options: {
		'dry-run': { type: 'boolean' },
		provenance: { type: 'boolean' },
		'expect-tag': { type: 'string' }
	}
});

const pkg_root = 'crates/tsv_napi/pkg';

const read_pkg = (dir: string): { name: string; version: string; [k: string]: unknown } => {
	try {
		return JSON.parse(Deno.readTextFileSync(`${dir}/package.json`));
	} catch (e) {
		console.error(`FAIL: cannot read ${dir}/package.json — is the set staged?`);
		console.error(String(e));
		Deno.exit(1);
	}
};

const loader = read_pkg(`${pkg_root}/napi`);
const version = loader.version;
const optional = loader.optionalDependencies as Record<string, string> | undefined;
if (!optional || Object.keys(optional).length === 0) {
	console.error('FAIL: loader package.json has no optionalDependencies — stale staging?');
	Deno.exit(1);
}

if (args['expect-tag'] && args['expect-tag'] !== `v${version}`) {
	console.error(
		`FAIL: staged version v${version} does not match the triggering tag ${args['expect-tag']} — ` +
			`the checkout and the staged artifacts disagree`
	);
	Deno.exit(1);
}

// Completeness: every platform package the loader pins must be staged, at the
// loader's own version, with its binary present. Partial sets never publish.
const platform_dirs: Array<{ name: string; dir: string }> = [];
let incomplete = false;
for (const [name, pin] of Object.entries(optional)) {
	const triple = name.replace('@fuzdev/tsv_napi-', '');
	const dir = `${pkg_root}/${triple}`;
	const exists = (() => {
		try {
			return Deno.statSync(`${dir}/tsv_napi.node`).isFile;
		} catch {
			return false;
		}
	})();
	if (!exists) {
		console.error(`MISSING: ${dir}/tsv_napi.node (${name}) — did its matrix job fail?`);
		incomplete = true;
		continue;
	}
	const pkg = read_pkg(dir);
	if (pkg.name !== name || pkg.version !== version || pin !== version) {
		console.error(
			`MISMATCH: ${dir} stages ${pkg.name}@${pkg.version}, loader pins ${name}@${pin}, ` +
				`loader version ${version}`
		);
		incomplete = true;
		continue;
	}
	platform_dirs.push({ name, dir });
}
if (incomplete) {
	console.error('FAIL: refusing to publish a partial set');
	Deno.exit(1);
}

const npm = (npm_args: string[]): { code: number; stdout: string; stderr: string } => {
	const out = new Deno.Command('npm', {
		args: npm_args,
		stdout: 'piped',
		stderr: 'piped'
	}).outputSync();
	return {
		code: out.code,
		stdout: new TextDecoder().decode(out.stdout).trim(),
		stderr: new TextDecoder().decode(out.stderr).trim()
	};
};

/** Whether `name@version` is already on the registry; non-404 errors are fatal.
 * An exit-0 answer with unexpected stdout reads as NOT published — the safe
 * degradation, since `npm publish` itself fails loudly on a real conflict,
 * where aborting here would kill a release on an npm output quirk. */
const already_published = (name: string): boolean => {
	const res = npm(['view', `${name}@${version}`, 'version']);
	if (res.code === 0) return res.stdout === version;
	if (res.stderr.includes('E404') || res.stderr.includes('404')) return false;
	console.error(`FAIL: npm view ${name}@${version} errored (not a 404):`);
	console.error(res.stderr || res.stdout);
	Deno.exit(1);
};

// Platforms first, loader last — the loader must never precede its binaries.
const to_publish = [...platform_dirs, { name: loader.name, dir: `${pkg_root}/napi` }];
let published = 0;
let skipped = 0;
for (const { name, dir } of to_publish) {
	if (already_published(name)) {
		console.log(`skip: ${name}@${version} already on the registry`);
		skipped++;
		continue;
	}
	const publish_args = [
		'publish',
		dir,
		'--access',
		'public',
		...(args.provenance ? ['--provenance'] : []),
		...(args['dry-run'] ? ['--dry-run'] : [])
	];
	console.log(`npm ${publish_args.join(' ')}`);
	const res = npm(publish_args);
	if (res.code !== 0) {
		console.error(`FAIL: npm publish ${name}@${version} exited ${res.code}:`);
		console.error(res.stderr || res.stdout);
		Deno.exit(1);
	}
	published++;
}
console.log(
	`${args['dry-run'] ? 'DRY-RUN: would publish' : 'published'} ${published}, skipped ${skipped} ` +
		`(of ${to_publish.length}) at v${version}`
);
