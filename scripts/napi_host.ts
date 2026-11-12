/**
 * The host machine's N-API platform triple, in node-platform terms
 * (`linux-x64-gnu`, `darwin-arm64`, `win32-x64`, …), from Deno's build target
 * (`host_triple`) or from what a Node process reports (`node_host_triple`).
 *
 * Its own module because several scripts read it: the staging
 * (`scripts/roots/build_napi_packages.ts`) picks the host triple when `--triple` is not given, and
 * `engine_parity.ts` picks the host's staged package over whatever `pkg/` happens to
 * list first — a runner that has gathered several matrix artifacts holds several.
 * Also home to `cli_binary_name`, the one spelling of the CLI binary's filename per
 * triple, and `release_asset_name`, the one spelling of its GitHub Release asset, for the
 * same reason.
 */

/** A runtime's build target in Deno's terms (its `build` record): what `host_triple` reads. */
export interface HostBuild {
	os: string;
	arch: string;
	target: string;
}

/** The host's N-API platform triple, from the build target the caller's runtime reports. */
export const host_triple = ({ os, arch, target }: HostBuild): string => {
	const cpu = arch === 'x86_64' ? 'x64' : arch === 'aarch64' ? 'arm64' : arch;
	if (os === 'linux') return `linux-${cpu}-${target.includes('musl') ? 'musl' : 'gnu'}`;
	if (os === 'darwin') return `darwin-${cpu}`;
	if (os === 'windows') return `win32-${cpu}`;
	return `${os}-${cpu}`;
};

/** What `node_host_triple` reads of a Node process: its `platform` and `arch`, and whether
 * its runtime reports a glibc version (the process report's `header.glibcVersionRuntime`,
 * absent on musl). */
export interface NodeHost {
	platform: string;
	arch: string;
	glibc: boolean;
}

/** The host's N-API platform triple from what a Node process reports of itself: Node's
 * platform and CPU names are the triples' own, and a Linux host is `gnu` exactly when its
 * runtime reports a glibc. */
export const node_host_triple = ({ platform, arch, glibc }: NodeHost): string =>
	platform === 'linux' ? `linux-${arch}-${glibc ? 'gnu' : 'musl'}` : `${platform}-${arch}`;

/** The native CLI binary's filename inside a platform package: `tsv.exe` on the
 * `win32-*` triples, `tsv` everywhere else — one spelling for every script that
 * stages, validates, publishes or re-fetches a platform package. */
export const cli_binary_name = (triple: string): string =>
	triple.startsWith('win32-') ? 'tsv.exe' : 'tsv';

/** The GitHub Release asset name of a triple's CLI binary: the binary's own name with the
 * triple spliced in, so the two keep one extension rule — `tsv` → `tsv-linux-x64-gnu`,
 * `tsv.exe` → `tsv-win32-x64.exe`. `scripts/release_assets.ts` names the assets with it, and
 * `package:audit` grades tsv_cli's binstall metadata against it. */
export const release_asset_name = (triple: string): string =>
	cli_binary_name(triple).replace(/^tsv/, `tsv-${triple}`);
