<script lang="ts">
	// trailing comment in a retained parenthesized intersection member
	type A1 = (a & b) /* c */ | c;

	// leading comment in a retained parenthesized intersection member
	type A2 = a | /* c */ (b & c);

	// trailing comment in a retained parenthesized intersection member (non-first)
	type A3 = a | (b & c) /* c */;
</script>
