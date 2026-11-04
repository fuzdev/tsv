<script lang="ts">
	x = x // a
		.y.q();
	const v = x // a
		.y.q(1);
	let w = x.y // a
		.z.q();
	x += x /* c */.y
		.q();
	class A {
		k = x // a
			.y[0]();
	}
	x = x
		// a
		.y.q();
	x = x
		// a
		/* b */
		.y.q();
	x = await x // a
		.y.q();
</script>
