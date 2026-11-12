/**
 * `doctor`'s root (`scripts/doctor.ts`). Its reach:
 *
 * - read-only handles on the checkout, found from this module's location, never the working
 *   directory; on its `target/`; and on each sibling checkout the doctor reads, each opened by
 *   its path beside the checkout, so a `target/` or a checkout that is a symlink to a directory
 *   elsewhere is followed
 * - the programs it probes, each found on `PATH` and run in the checkout: `deno`, `bun`,
 *   `wasm-pack`, and `npm` with gro's base environment (`PATH`, `HOME`, `TMPDIR`, `TERM`,
 *   `LC_ALL=C`, `NO_COLOR`, `FORCE_COLOR`), so an `npm_config_*` an `npm run` sets steers no
 *   `npm`; `cargo` with rustup's and cargo's homes too (`RUSTUP_*`, `CARGO_HOME`), which pick
 *   the toolchain beside the checkout's `rust-toolchain.toml`; `node` and `git` with git's local
 *   variables (`GIT_LOCAL_PASS_ENV`), since the canonical pin audit `node` runs reads the
 *   checkouts' commits with `git`
 * - the bench modules it reports through, which read with `node:fs` (Bun runs them too): the
 *   bench install's probe, the oracle pins, and the corpus loader, loaded only when its section
 *   runs, from the working directory as the loader resolves its paths, so run it from the
 *   checkout's root, as the npm script does
 *
 * It writes nothing.
 *
 * @module
 */

import { dirname, join } from 'node:path';

import { fs, type FsReadDir } from '@fuzdev/gro/fs.ts';
import { GIT_LOCAL_PASS_ENV } from '@fuzdev/gro/git.ts';
import { process } from '@fuzdev/gro/process.ts';
import { spawn, type SpawnProgram, type SpawnProgramOptions } from '@fuzdev/gro/spawn.ts';
import { is_spawn_error } from '@fuzdev/gro/spawn_error.ts';

import { probe_node_modules } from '../../benches/js/lib/check_node_modules.ts';
import { load_all_versions } from '../../benches/js/lib/versions.ts';
import {
	DOCTOR_SIBLINGS,
	type DoctorDir,
	type DoctorProgram,
	type DoctorSibling,
	run_doctor
} from '../doctor.ts';
import { open_checkout } from './checkout.ts';

/** The directory at `path`, read-only, or `null` when it can't be opened, as a doctor reads an
 * absence. */
const open_readonly = async (path: string): Promise<FsReadDir | null> => {
	try {
		return (await fs.open_dir(path)).readonly();
	} catch {
		return null;
	}
};

const checkout = (await open_checkout()).readonly();

/** `name` on `PATH`, run in the checkout, or `null` when it isn't there to run. */
const find = (name: string, options?: SpawnProgramOptions): DoctorProgram | null => {
	const program = find_program(name, options);
	return program && { out: (args) => program.out(args, { cwd: checkout }) };
};

/** `spawn.find`, with a file of that name the kernel can't run read as no program either. */
const find_program = (name: string, options?: SpawnProgramOptions): SpawnProgram | null => {
	try {
		return spawn.find(name, options);
	} catch (error) {
		if (is_spawn_error(error)) return null;
		throw error;
	}
};

const siblings = {} as Record<DoctorSibling, DoctorDir | null>;
for (const name of DOCTOR_SIBLINGS) {
	siblings[name] = await open_readonly(join(dirname(checkout.path), name));
}

const code = await run_doctor(
	{
		checkout,
		target: await open_readonly(join(checkout.path, 'target')),
		siblings,
		programs: {
			deno: find('deno'),
			node: find('node', { pass_env: GIT_LOCAL_PASS_ENV }),
			bun: find('bun'),
			cargo: find('cargo', { pass_env: ['RUSTUP_*', 'CARGO_HOME'] }),
			wasm_pack: find('wasm-pack'),
			npm: find('npm'),
			git: find('git', { pass_env: GIT_LOCAL_PASS_ENV })
		},
		probe_node_modules,
		load_all_versions,
		load_corpus: () => import('../../benches/js/lib/corpus.ts')
	},
	process.args
);
process.set_exit_code(code);
