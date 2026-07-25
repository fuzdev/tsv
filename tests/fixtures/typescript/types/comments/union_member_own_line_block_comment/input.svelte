<script lang="ts">
	// an own-line block comment before a union member holds the union broken and stays
	// own-line above the member's `|` (matches prettier)
	type A =
		| a
		/* c1 */
		| { x: 1 }
		| b;

	// a glued-authored inline block comment stays glued and the union stays flat —
	// two authorings, two fixed points (both prettier-stable)
	type B = a | /* c2 */ { y: 2 } | b;
</script>
