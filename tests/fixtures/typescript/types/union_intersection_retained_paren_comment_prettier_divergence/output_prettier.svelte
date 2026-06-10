<script lang="ts">
	// trailing comment inside a retained parenthesized union member
	type A1 = a | (b | c) /* c */;

	// leading comment inside a retained parenthesized union member
	type A2 = a | /* c */ (b | c);

	// retained parenthesized union as the first member
	type A3 = (a | b) /* c */ | c;

	// retained parenthesized union as an intersection member
	type A4 = a & (b | c) /* c */;
</script>
