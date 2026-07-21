<script lang="ts">
	// trailing line comment inside a retained parenthesized union member (first member)
	type A1 =
		| (
				a | b // c
		  )
		| c;

	// trailing line comment inside a retained parenthesized union member (middle member)
	type A2 =
		| a
		| (
				b | c // c
		  )
		| d;
</script>
