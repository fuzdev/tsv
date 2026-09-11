<script lang="ts">
	// a line comment trails the comma and the next operand's multi-line comment keeps its own
	// line: the operand keeps its own layout
	x =
		(a, // x
		/* y
		 */ aaa ? bbb : ccc);

	// a preserved multi-line comment: same rule
	x =
		(a, // x
		/* y
z */ aaa ? bbb : ccc);

	// a middle operand
	x =
		(a, // x
		/* y
		 */ aaa ? bbb : ccc,
		d);

	// a call argument
	fn(
		(a, // x
		/* y
		 */ aaa ? bbb : ccc)
	);

	// variable initializer
	const v =
		(a, // x
		/* y
		 */ aaa ? bbb : ccc);

	// return argument
	function f() {
		return (
			a, // x
			/* y
			 */ aaa ? bbb : ccc
		);
	}
</script>
