<script lang="ts">
	// Line comment in the gap between a call and the `[` of a NUMERIC computed access.
	// The comment ends its line, so the chain breaks and the `[` drops to its own line.
	const a = foo
		.bar()
		.baz()
		// c1
		[0];

	// Two comments in that gap stay distinct and in order.
	const b = foo
		.bar()
		.baz()
		// c2
		// c3
		[0];

	// A same-line comment trails the call; the bracket still drops.
	const c = foo
		.bar()
		.baz() // c4
		[0];

	// The same rule on a SHORT chain — a lone call base, an identifier base, a same-line
	// comment, and statement position: the comment and the bracket indent one level.
	const d = fn()
		// c5
		[0];
	const e = arr
		// c6
		[0];
	const f = fn() // c7
		[0];
	fn()
		// c8
		[0];
</script>
