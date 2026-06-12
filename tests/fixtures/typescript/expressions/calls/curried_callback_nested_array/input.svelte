<script lang="ts">
	// Two nested arrays - outer call should stay hugged
	fn([
		[1, 2],
		[3, 4],
	])('name', () => {
		x();
	});

	// Real-world: test.each pattern
	test.each([
		[1, 1, 2],
		[1, 2, 3],
	])('add(%i, %i) = %i', (a, b, expected) => {
		expect(a + b).toBe(expected);
	});
</script>
