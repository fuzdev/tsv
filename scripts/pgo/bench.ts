/**
 * The bench's profile-guided native pair — `deno task build:bench:pgo`, which every bench
 * build runs (`build:bench`). The bench measures what ships: every native tsv row loads a
 * profile-guided build (docs/benchmarks.md §PGO), the N-API addon from the shipped addon's
 * own pipeline and the C-FFI library from the same pipeline's third world.
 *
 * It runs that pipeline (`scripts/pgo/pgo.ts all --ffi --reuse`, its working directory
 * `.pgo/`, shared with `deno task build:pgo`), which skips the build and its gates when its
 * stamp says the binaries there were built and gated from exactly this run's inputs — so a
 * bench build reuses what `build:pgo` produced and the other way round. The depth gate is
 * the shipped addon's release check, not a measurement one, so it is skipped here; the gates
 * that guard what the bench measures (a non-empty profile applied to every crate, no
 * instrumentation in the binaries, output byte-identical to the plain builds) are the
 * pipeline's, passed for these bytes or the pipeline fails. `TSV_BUILD_FORCE=1` drops
 * `--reuse`, rebuilding regardless — what a change the key cannot see needs.
 *
 * Then it copies the two libraries into the bench's slot and writes the bench stamp — the
 * stamp removed first and written last, so an interrupted copy leaves none that vouches for
 * it. The copy is made on every run, so a library's mtime says when its build was last
 * confirmed current, the reading the bench's mtime guard needs. A side effect worth knowing:
 * the pipeline stages its PGO pair into `crates/tsv_napi/pkg/<triple>/`, as `build:pgo` does.
 *
 * Usage: deno task build:bench:pgo
 *
 * @module
 */

import { copyFileSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';

import { CORPORA_ROOT } from '../../benches/js/lib/corpora.ts';
import { GATE_CHECKOUT_IDS } from '../../benches/js/lib/gate_counts.ts';
import {
	PGO_BENCH_DIR,
	PGO_BENCH_STAMP_PATH,
	PGO_BENCH_STAMP_VERSION,
	PGO_BENCH_WORLDS,
	type PgoBenchStamp,
	type PgoBenchWorld,
	pgo_bench_library_path,
	sha256_file
} from '../../benches/js/lib/pgo_bench.ts';
import { host_triple } from '../napi_host.ts';
import { cargo_artifacts, pgo_paths } from './layout.ts';

/** The pipeline's working directory — `pgo.ts`'s default, so `build:pgo` shares it. */
const PGO_DIR = '.pgo';

/** What each world's profile was trained through, as the report discloses it. */
const TRAINED_THROUGH: Record<PgoBenchWorld, string> = {
	ffi:
		'the library itself, in process under Deno through the bench’s own FFI wrapper — ' +
		'parse, parse_internal and format on every file',
	napi:
		'the addon itself — parse on every file and format --jobs 1 over the tree through the ' +
		'@fuzdev/tsv loader’s cli.js, the type strip and import records in process — the ' +
		'training the published addon gets'
};

const force = Deno.env.get('TSV_BUILD_FORCE') === '1';
const pipeline = await new Deno.Command(Deno.execPath(), {
	args: [
		'run',
		'--allow-read',
		'--allow-write',
		'--allow-env',
		'--allow-run',
		'--allow-sys',
		'scripts/pgo/pgo.ts',
		'all',
		'--ffi',
		'--no-depth',
		...(force ? [] : ['--reuse']),
		'--dir',
		PGO_DIR
	],
	stdin: 'inherit',
	stdout: 'inherit',
	stderr: 'inherit'
}).output();
if (!pipeline.success) {
	console.error(`build:bench:pgo: the PGO pipeline failed (exit ${pipeline.code}); nothing staged`);
	Deno.exit(1);
}

const paths = pgo_paths(PGO_DIR);
const built = cargo_artifacts(paths.target('use'), host_triple());
const source: Record<PgoBenchWorld, string> = { ffi: built.ffi, napi: built.addon };
const corpus = JSON.parse(Deno.readTextFileSync(paths.corpus)) as { own: string[] };

// the pipeline's own stamp names what its gates passed; the bench stamp may name nothing else
const gated = JSON.parse(Deno.readTextFileSync(paths.stamp)) as {
	artifacts: Record<string, { sha256: string; profile_sha256: string }>;
};

rmSync(PGO_BENCH_STAMP_PATH, { force: true });
mkdirSync(PGO_BENCH_DIR, { recursive: true });
const artifacts = {} as PgoBenchStamp['artifacts'];
for (const world of PGO_BENCH_WORLDS) {
	const dest = pgo_bench_library_path(world);
	// a fresh file rather than an overwrite in place: a process that has the old library
	// loaded keeps its own mapping
	rmSync(dest, { force: true });
	copyFileSync(source[world], dest);
	artifacts[world] = {
		sha256: sha256_file(dest),
		profile_sha256: sha256_file(paths.profdata(world)),
		trained_through: TRAINED_THROUGH[world]
	};
	const passed = gated.artifacts[world];
	if (
		passed === undefined ||
		passed.sha256 !== artifacts[world].sha256 ||
		passed.profile_sha256 !== artifacts[world].profile_sha256
	) {
		console.error(
			`build:bench:pgo: the ${world} library or profile on disk is not what ${paths.stamp} ` +
				'records the gates passing — nothing staged'
		);
		rmSync(dest, { force: true });
		Deno.exit(1);
	}
}
const stamp: PgoBenchStamp = {
	version: PGO_BENCH_STAMP_VERSION,
	generated: new Date().toISOString(),
	training: {
		origin: 'own',
		collections: corpus.own,
		snapshot_tree: GATE_CHECKOUT_IDS[CORPORA_ROOT]!.hash
	},
	artifacts
};
writeFileSync(PGO_BENCH_STAMP_PATH, JSON.stringify(stamp, null, '\t') + '\n');
console.log(`build:bench:pgo: staged the PGO FFI library and N-API addon in ${PGO_BENCH_DIR}`);
