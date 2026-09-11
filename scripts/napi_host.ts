/**
 * The host machine's N-API platform triple, in node-platform terms
 * (`linux-x64-gnu`, `darwin-arm64`, `win32-x64`, …), from Deno's own build target.
 *
 * Its own module because two scripts read it and the one that stages the packages
 * (`build_napi_packages.ts`) runs its staging on import, so it can export nothing:
 * the staging picks the host triple when `--triple` is not given, and
 * `engine_parity.ts` picks the host's staged package over whatever `pkg/` happens to
 * list first — a runner that has gathered several matrix artifacts holds several.
 */
export const host_triple = (): string => {
	const { os, arch, target } = Deno.build;
	const cpu = arch === 'x86_64' ? 'x64' : arch === 'aarch64' ? 'arm64' : arch;
	if (os === 'linux') return `linux-${cpu}-${target.includes('musl') ? 'musl' : 'gnu'}`;
	if (os === 'darwin') return `darwin-${cpu}`;
	if (os === 'windows') return `win32-${cpu}`;
	return `${os}-${cpu}`;
};
