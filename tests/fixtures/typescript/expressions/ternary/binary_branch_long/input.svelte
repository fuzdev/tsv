<script>
	// Binary chain in return ternary alternate - fits at 100 (inline)
	function fn1(a) {
		return a < 0.5
			? a * bbbbb(cccccccccc, ddddddddddddddddd) * eeeee(ffffffffffff, ggggggggggggggggggggg) + 1.0
			: a * bbbbb(cccccccccc, ddddddddddddddddd) * eeeee(ffffffffffff, ggggggggggggggggggggg) + 1.0;
	}

	// Binary chain in return ternary alternate - breaks at 101 (continuation double-indented)
	function fn2(a) {
		return a < 0.5
			? a * bbbbb(ccccccccccc, ddddddddddddddddd) * eeeee(ffffffffffff, ggggggggggggggggggggg) + 1.0
			: a * bbbbb(ccccccccccc, ddddddddddddddddd) * eeeee(ffffffffffff, ggggggggggggggggggggg) +
					1.0;
	}
</script>
