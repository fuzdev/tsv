<script lang="ts">
	// an own-line block before a plain member is glued between the object and the member
	a /* c */.b;
	const v1 = a /* c */?.b;
	v2 = a /* c */.b;
	const fn = () => a /* c */.b;

	// a multi-line block glues the same way
	const v3 =
		a /* m
	n */.b;

	// a member the chain's head absorbs ahead of a call
	a /* c */.b
		.m(1);
</script>
