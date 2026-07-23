<script lang="ts">
	// A line comment leading, a block comment trailing the cast type; the block
	// defers past the statement `;`.
	const a = x as A /* t */; // c

	const b = x satisfies B /* t */; // c
</script>
