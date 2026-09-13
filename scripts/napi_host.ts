/**
 * The host machine's N-API platform triple, in node-platform terms
 * (`linux-x64-gnu`, `darwin-arm64`, `win32-x64`, …), from Deno's own build target.
 *
 * Its own module because several scripts read it and the one that stages the
 * packages (`build_napi_packages.ts`) runs its staging on import, so it can export
 * nothing: the staging picks the host triple when `--triple` is not given, and
 * `engine_parity.ts` picks the host's staged package over whatever `pkg/` happens to
 * list first — a runner that has gathered several matrix artifacts holds several.
 * Also home to `cli_binary_name`, the one spelling of the CLI binary's filename per
 * triple, for the same reason.
 */
export const host_triple = (): string => {
	const { os, arch, target } = Deno.build;
	const cpu = arch === 'x86_64' ? 'x64' : arch === 'aarch64' ? 'arm64' : arch;
	if (os === 'linux') return `linux-${cpu}-${target.includes('musl') ? 'musl' : 'gnu'}`;
	if (os === 'darwin') return `darwin-${cpu}`;
	if (os === 'windows') return `win32-${cpu}`;
	return `${os}-${cpu}`;
};

/** The native CLI binary's filename inside a platform package: `tsv.exe` on the
 * `win32-*` triples, `tsv` everywhere else — one spelling for every script that
 * stages, validates, publishes or re-fetches a platform package. */
export const cli_binary_name = (triple: string): string =>
	triple.startsWith('win32-') ? 'tsv.exe' : 'tsv';
