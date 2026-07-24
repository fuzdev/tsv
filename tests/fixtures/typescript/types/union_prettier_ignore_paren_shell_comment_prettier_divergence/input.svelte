<script lang="ts">
	// a frozen paren-wrapped member is kept verbatim WITH its redundant paren, so a comment
	// in the paren shell survives; prettier strips the paren and relocates the comment out
	type A =
		// prettier-ignore
		(/* keep */ a1) | b;

	// same lossless-shell rule at a between-members position
	type B =
		| a
		// prettier-ignore
		| (/* cB */ b1);
</script>
