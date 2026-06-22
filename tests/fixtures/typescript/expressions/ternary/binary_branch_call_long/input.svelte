<script>
	// Binary chain in call arg ternary alternate - fits at 100 (inline)
	const x1 = fn(
		a < 0.5
			? a * bbbbb(ccccccccccc, ddddddddddddddddd) * eeeee(ffffffffffff, ggggggggggggggggggggg) + 1.0
			: a * bbbbb(ccccccccccc, ddddddddddddddddd) * eeeee(ffffffffffff, ggggggggggggggggggggg) + 1.0
	);

	// Binary chain in call arg ternary alternate - breaks at 101 (continuation double-indented)
	const x2 = fn(
		a < 0.5
			? a * bbbbb(cccccccccccc, ddddddddddddddddd) * eeeee(ffffffffffff, ggggggggggggggggggggg) +
					1.0
			: a * bbbbb(cccccccccccc, ddddddddddddddddd) * eeeee(ffffffffffff, ggggggggggggggggggggg) +
					1.0
	);
</script>
