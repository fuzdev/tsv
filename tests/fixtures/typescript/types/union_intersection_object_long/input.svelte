<script lang="ts">
	// Multiline object in parenthesized intersection
	// Uses tabs + spaces for alignment
	type T =
		| (A & {
				prop1: Type1;
				prop2: Type2;
		  })
		| B;
</script>
