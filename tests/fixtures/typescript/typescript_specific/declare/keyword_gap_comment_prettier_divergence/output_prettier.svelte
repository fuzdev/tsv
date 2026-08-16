<script lang="ts">
	// A block comment between `declare` and the head keyword stays after `declare` —
	// it annotates the ambient-ness, and the same rule reaches every head
	declare namespace /* a1 */ A {}
	declare module /* a2 */ B {}
	declare module /* a3 */ 'c' {}
	declare interface /* a4 */ D {}
	declare enum /* a5 */ E {}
	declare type /* a6 */ F = number;
	declare class /* a7 */ G {}
	declare function /* a8 */ fn(): void;
	declare const /* a9 */ h: number;

	// The two interior gaps are separate positions and each keeps its own comment
	declare namespace /* b1 */ /* c1 */ I {}
	declare module /* b2 */ /* c2 */ J {}
	declare interface /* b3 */ /* c3 */ K {}
	declare enum /* b4 */ /* c4 */ L {}
	declare type /* b5 */ /* c5 */ M = number;
	declare abstract class /* b6 */ /* c6 */ N {}

	// `const enum` is the one three-word head, so it has three positions and all of
	// them stay distinct
	const enum /* d1 */ O {}
	declare const enum /* d2 */ /* d3 */ /* d4 */ P {}

	// `const`→`enum` carries no `[no LineTerminator here]`, so it is the one gap here
	// a *line* comment reaches: it takes the uniform forced-continuation indent
	const enum // d5
	R {}
	declare const enum // d6
	S {}

	// `global` is both keyword and name, so it has only the one gap — and prettier
	// keeps this comment where the author wrote it too
	declare /* e */ global {}

	// The same gap behind `export`
	export declare namespace /* f */ Q {}
</script>
