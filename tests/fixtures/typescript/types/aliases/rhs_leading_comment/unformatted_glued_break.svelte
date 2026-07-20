<script lang="ts">
	// Glued single-line block that fits: stays inline on the `=` line.
	type Glued =
		/* c */ Ref;

	// Block comment alone on its own line: kept there, RHS renders below — even
	// though the whole alias would fit on one line.
	type OwnBlock =
		/* c */
		Ref;

	// Line comment alone on its own line: same, RHS below.
	type OwnLine =
		// c
		Ref;
</script>
