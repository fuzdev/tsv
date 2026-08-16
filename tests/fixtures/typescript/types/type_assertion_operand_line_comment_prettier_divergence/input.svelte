<script lang="ts">
	// A line comment cannot trail inline before the `)`, so the operand's parens
	// are retained and the shell goes multiline with the comment inside it.
	const a = <T>(
		p // c
	);

	// An operand whose parens are required takes the same single pair.
	const b = <T>(
		m + n // c
	);

	// A block glued behind the `//` is part of the same run and keeps its order.
	const c = <T>(
		q /* c1 */ // c2
	);
</script>
