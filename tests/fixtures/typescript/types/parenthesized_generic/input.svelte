<script lang="ts">
	// Parenthesized generic type
	type A = Array<string>;

	// Parenthesized generic with multiple type args
	type B = Map<string, number>;

	// Parenthesized generic in union
	type C = Array<string> | null;
</script>
