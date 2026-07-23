<script lang="ts">
	// A block comment then a line comment before the cast type.
	const a = x /* b */ as A; // c

	const b = x /* b */ satisfies B; // c
</script>
