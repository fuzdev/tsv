<script lang="ts">
	// A block comment between `declare` and the head keyword stays after `declare` —
	// it annotates the ambient-ness, and the same rule reaches every head
	declare /* a1 */ namespace A {}
	declare /* a2 */ module B {}
	declare /* a3 */ module 'c' {}
	declare /* a4 */ interface D {}
	declare /* a5 */ enum E {}
	declare /* a6 */ type F = number;
	declare /* a7 */ class G {}
	declare /* a8 */ function fn(): void;
	declare /* a9 */ const h: number;

	// The two interior gaps are separate positions and each keeps its own comment
	declare /* b1 */ namespace /* c1 */ I {}
	declare /* b2 */ module /* c2 */ J {}
	declare /* b3 */ interface /* c3 */ K {}
	declare /* b4 */ enum /* c4 */ L {}
	declare /* b5 */ type /* c5 */ M = number;
	declare /* b6 */ abstract /* c6 */ class N {}

	// `const enum` is the one three-word head, so it has three positions and all of
	// them stay distinct
	const /* d1 */ enum O {}
	declare /* d2 */ const /* d3 */ enum /* d4 */ P {}

	// `const`→`enum` carries no `[no LineTerminator here]`, so it is the one gap here
	// a *line* comment reaches: it takes the uniform forced-continuation indent
	const // d5
		enum R {}
	declare const // d6
		enum S {}

	// `global` is both keyword and name, so it has only the one gap — and prettier
	// keeps this comment where the author wrote it too
	declare /* e */ global {}

	// The same gap behind `export`
	export declare /* f */ namespace Q {}
</script>
