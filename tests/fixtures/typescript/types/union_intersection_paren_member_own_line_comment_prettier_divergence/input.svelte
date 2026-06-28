<script lang="ts">
	// own-line line comment inside a retained parenthesized union member of an intersection
	type T = (a | b) &
		(
			// c
			a | b
		);

	// same, in a 3-member intersection — the middle paren member keeps its comment
	type U = (a | b) &
		(
			// c
			a | b
		) &
		(c | d);
</script>
