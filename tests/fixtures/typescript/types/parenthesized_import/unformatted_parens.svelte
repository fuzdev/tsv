<script lang="ts">
	// Parenthesized import type
	type A = (import("./a").B);

	// Parenthesized import type in union
	type C = (import("./a").B | null);
</script>
