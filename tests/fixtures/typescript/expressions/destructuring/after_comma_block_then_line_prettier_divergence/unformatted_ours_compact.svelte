<script>
	// a block after the comma stays on the comma line, in front of the line comment the
	// author wrote after it
	const { a1,/* c1 */// c2
b1 } = o;

	// same placement in an array pattern
	const [a2,/* c1 */// c2
b2] = o;

	// a run of blocks after the comma keeps its order
	const { a3,/* c1 *//* c2 */// c3
b3 } = o;

	// blocks on both sides of the comma each keep their side of it
	const { a4/* c0 */,/* c1 */// c2
b4 } = o;
</script>
