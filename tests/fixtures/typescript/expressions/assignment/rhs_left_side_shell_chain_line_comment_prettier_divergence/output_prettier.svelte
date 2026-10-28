<script lang="ts">
	// An own-line comment in the grouping parens of a value's leftmost node hangs the value under
	// the operator, as the comment does in the operator's own gap, also where the value's chain
	// carries a line comment of its own.
	x =
		// c1
		x.a() // c2
		[x];
	const y =
		// c3
		x.a() // c4
		[x];
	class K {
		p =
			// c5
			x.a() // c6
			[x];
	}
</script>
