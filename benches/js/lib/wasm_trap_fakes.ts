/**
 * A real WebAssembly trap for the suites that grade crash classification: a module assembled
 * here byte by byte, so a suite raises the `WebAssembly.RuntimeError` a `panic = "abort"`
 * tsv-wasm build raises on a panic, under the suite's own runtime, with no build step and no
 * package. Test support only, imported by the `*_test.ts` suites.
 *
 * @module
 */

/**
 * The module: two functions, each of which traps. `unreachable` executes the `unreachable`
 * instruction (what a `panic = "abort"` Rust build's panic compiles to); `divide` divides by
 * zero. Two traps with two messages, one type.
 */
const TRAP_MODULE = new Uint8Array([
	...[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00], // magic, version 1
	...[0x01, 0x04, 0x01, 0x60, 0x00, 0x00], // types: () -> ()
	...[0x03, 0x03, 0x02, 0x00, 0x00], // functions: two of type 0
	...[0x07, 0x18, 0x02], // exports: two
	...[0x0b, ...new TextEncoder().encode('unreachable'), 0x00, 0x00], // "unreachable" = func 0
	...[0x06, ...new TextEncoder().encode('divide'), 0x00, 0x01], // "divide" = func 1
	...[0x0a, 0x0e, 0x02], // code: two bodies
	...[0x03, 0x00, 0x00, 0x0b], // no locals; unreachable; end
	...[0x08, 0x00, 0x41, 0x01, 0x41, 0x00, 0x6d, 0x1a, 0x0b] // no locals; 1 / 0; drop; end
]);

const traps = new WebAssembly.Instance(new WebAssembly.Module(TRAP_MODULE)).exports as Record<
	'unreachable' | 'divide',
	() => void
>;

/**
 * Run one of the module's functions, which traps: the throw is the engine's own
 * `WebAssembly.RuntimeError` (message `unreachable` or `divide by zero`).
 * @param trap - which function: the `unreachable` instruction, or a division by zero
 */
export function raise_wasm_trap(trap: 'unreachable' | 'divide' = 'unreachable'): never {
	traps[trap]();
	throw new Error(`the trap module's ${trap} returned`);
}
