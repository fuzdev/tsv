<script lang="ts">
	// An own-line comment in the value→`;` gap drops to its own line below the `;`
	type A = B;
	/* a */

	// An author blank above such a comment survives the terminator
	type C = D;

	/* c */
</script>
