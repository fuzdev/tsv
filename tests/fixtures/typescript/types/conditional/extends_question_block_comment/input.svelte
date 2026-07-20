<script lang="ts">
	// Single-line block comment in the extends-type -> `?` gap: stays flat, comment
	// preserved after the extends type.
	type A = B extends C /* c */ ? D : E;

	// Two single-line blocks in the same gap: still flat, both preserved.
	type F = B extends C /* c1 */ /* c2 */ ? D : E;

	// Multiline block in that gap: hangs the branches (the comment ends its line).
	type G = B extends C /* c
	 more */
		? D
		: E;

	// Line comment in that gap: hangs the branches (a `//` ends its line).
	type H = B extends C // c
		? D
		: E;

	// All four gaps at once: each comment stays in the gap it was written in.
	type I = B extends C /* c1 */ ? /* c2 */ D /* c3 */ : /* c4 */ E;

	// Nested conditional in the true branch: the outer gap's comment still collapses.
	type J = B extends C /* c */ ? (D extends E ? F : G) : H;

	// Chained conditional: the comment sits in the inner conditional's gap.
	type K = B extends C ? D : E extends F /* c */ ? G : H;

	// Control: no comment stays flat.
	type L = B extends C ? D : E;

	// Control: the sibling `?` -> branch gap already collapses inline.
	type M = B extends C ? /* c */ D : E;
</script>
