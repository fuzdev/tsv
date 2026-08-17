<script lang="ts">
	// at exactly 100 the whole declaration fits, so nothing breaks
	const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: T = new A({});

	// at 101 an empty container argument keeps softlines, so the call breaks its own
	// parens rather than the assignment breaking at =
	const bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: T = new A(
		{}
	);
	const ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc: T = new A(
		[]
	);

	// control: the plain call breaks the same way
	const ddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd: T = fn1(
		{}
	);

	// a container behind a type cast hugs, and expands inside the cast
	new A({
		b: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
	} as T);

	// control: the plain call hugs the same way
	fn1({
		b: fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
	} as T);
</script>
