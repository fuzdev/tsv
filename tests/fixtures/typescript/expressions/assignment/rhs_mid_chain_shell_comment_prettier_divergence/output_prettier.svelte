<script lang="ts">
	x = x.y // a
		.q();
	const v = x.y // a
		.q(1);
	let w = x.y.z // a
		.q();
	x += x /* c */.y
		.q();
	class A {
		k = x.y[0](); // a
	}
	x = x// a
	.y
		.q();
	x = x// a
	/* b */
	.y
		.q();
	x = await x.y // a
		.q();
</script>
