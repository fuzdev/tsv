/**
 * The `Deno` global, untyped, for `tsconfig.vitest.json`'s program.
 *
 * Some modules the vitest suites import still call `Deno.*` inside functions those suites
 * never reach. `deno check` grades those calls against Deno's own types (`typecheck:js:deno`
 * walks `benches/js` and `scripts/`); here `Deno` reads as `any`, so `tsc` grades the rest of
 * each module without restating Deno's API.
 * The namespace holds the names those modules use in type positions.
 *
 * @module
 */

declare namespace Deno {
	type ChildProcess = any;
	type DynamicLibrary<_Symbols> = any;
	type PointerValue = any;
	type UnsafePointerView = any;
}

declare var Deno: any;
