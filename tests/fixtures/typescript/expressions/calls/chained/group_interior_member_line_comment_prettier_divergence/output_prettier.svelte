<script lang="ts">
	// a same-line comment after the base call, two members following: the members
	// start their own group and the chain breaks around the comment
	fn() // c1
	.bar.baz;

	// two gaps with a comment each, the chain called at its end: each in place
	fn() // c2
		.bar // c3
		.baz();

	// an own-line comment before the members keeps its line
	// c4
	fn().bar.baz;

	// after a member the base group would otherwise absorb
	fn()
		.bar // c5
		.baz.qux();

	// the factory / `this` merge is refused when the merged member's gap holds one
	Object.keys(x) // c6
		.map(f);
	this.x // c7
		.y();

	// in an initializer, behind a statement trailer
	const a =
		fn() // c8
		.bar.baz; // c9
</script>
