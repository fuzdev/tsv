<script lang="ts">
	// A line comment leading, a block comment trailing the cast type; the block
	// defers past the statement `;`.
	const a = x as (// c
		A /* t */);

	const b = x satisfies (// c
		B /* t */);
</script>
