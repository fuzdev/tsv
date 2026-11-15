/**
 * `publish:crates`' root (`scripts/publish_crates.ts`). Its reach:
 *
 * - the checkout read-only, found from this module's location, cargo's and git's working
 *   directory
 * - `cargo` with cargo's variables (`CARGO_PASS_ENV`), which carry a `CARGO_REGISTRY_TOKEN`
 *   on a publish
 * - `git` with git's local variables (`GIT_LOCAL_PASS_ENV`): a `--wetrun` reads HEAD, the
 *   version's tag, and `git status` there
 * - the crates.io API, and no other host
 * - writes beneath `target/package` and `$HOME/.cargo/registry/src` alone, opened when the
 *   cleanup runs (`scripts/roots/package_state.ts`)
 *
 * Either program missing from `PATH` fails only the call that needs it, as the library says.
 *
 * @module
 */

import { GIT_LOCAL_PASS_ENV } from '@fuzdev/gro/git.ts';
import { net } from '@fuzdev/gro/net.ts';
import { env } from '@fuzdev/gro/env.ts';
import { stderr } from '@fuzdev/gro/stderr.ts';
import { args } from '@fuzdev/gro/args.ts';
import { set_exit_code } from '@fuzdev/gro/exit.ts';
import { spawn } from '@fuzdev/gro/spawn.ts';

import { CARGO_PASS_ENV } from '../cargo_env.ts';
import { run_publish_crates } from '../publish_crates.ts';
import { open_checkout } from './checkout.ts';
import {
	open_package_state,
	PACKAGE_STATE_ENV_NAMES,
	type PackageStateEnv,
	to_registry_src_path
} from './package_state.ts';

const env_handle = env.narrow(PACKAGE_STATE_ENV_NAMES);
const package_env: PackageStateEnv = Object.fromEntries(
	PACKAGE_STATE_ENV_NAMES.map((name) => [name, env_handle.get(name)])
);

const checkout = await open_checkout();
const { fetch } = net.narrow({ hosts: ['crates.io'] });
const code = await run_publish_crates(
	{
		checkout: checkout.readonly(),
		cargo: spawn.find('cargo', { pass_env: CARGO_PASS_ENV }),
		git: spawn.find('git', { pass_env: GIT_LOCAL_PASS_ENV }),
		registry: { fetch },
		registry_src_path: to_registry_src_path(package_env),
		open_package_state: (target_directory) =>
			open_package_state({ checkout: checkout.path, target_directory, env: package_env }),
		print: (line) => console.log(line),
		print_error: (line) => console.error(line),
		write_error: (bytes) => stderr.write(bytes),
		sleep: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
		now: () => Date.now()
	},
	args
);
set_exit_code(code);
