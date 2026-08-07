<script>
	// a block comment past the last property takes its own line before the closer
	const o1 = { a: (
1
)
/* c */ };

	// a line comment in the same position
	const o2 = { a: (
1
)
// c
};
</script>
