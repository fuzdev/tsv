<script lang="ts">
	// a run of line comments in a retained intersection shell's trailing gap — each
	// comment keeps its own line, at the `)` column
	type A1 =
		| (a & { x: X }) // c1
		// c2
		| c;

	// three comments in the run, same rule
	type A2 =
		| (a & { x: X }) // c1
		// c2
		// c3
		| c;

	// a block comment after a line comment keeps the authored order — the line comment
	// cannot share its line, so the block takes the next one
	type A3 =
		| (a & { x: X }) // c1
		/* c2 */
		| c;

	// two glued block comments share a line — a block comment can, so no break is added
	type A4 = (a & { x: X }) /* c1 */ /* c2 */ | c;

	// the union shell's trailing gap takes the run at its interior indent
	type B1 =
		| (
				| a
				| b // c1
		  )
		// c2
		| c;

	// and the same run in a union shell that is an array element
	type B2 = (
		| a
		| b // c1
		// c2
	)[];
</script>
