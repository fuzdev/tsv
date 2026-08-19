<script lang="ts">
	// A paren the frozen value does not need is stripped, and a block comment written
	// inside it defers past the terminator — no `)` survives for it to cross.
	const aaa =
		// prettier-ignore
		bbb  +  ccc; /* c */

	// A line comment retains that same shell on its own account: deferred past the `;` it
	// would swallow whatever follows on that line.
	const ddd =
		// prettier-ignore
		eee  +  fff; // c
</script>
