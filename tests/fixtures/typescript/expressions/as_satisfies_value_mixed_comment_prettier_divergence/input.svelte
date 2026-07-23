<script lang="ts">
	// A block comment then a line comment before the cast type.
	const a = x as /* b */ // c
		A;

	const b = x satisfies /* b */ // c
		B;
</script>
