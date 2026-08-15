<script lang="ts">
	// a same-line comment after the base call, two members following: the members
	// start their own group and the chain breaks around the comment
	fn()// c1
	.bar.baz;

	// two gaps with a comment each, the chain called at its end: each in place
	fn()// c2
	.bar// c3
	.baz();

	// an own-line comment before the members keeps its line
	fn()
	// c4
	.bar.baz;

	// after a member the base group would otherwise absorb
	fn().bar// c5
	.baz.qux();

	// the factory / `this` merge is refused when the merged member's gap holds one
	Object// c6
	.keys(x)
	.map(f);
	this// c7
	.x.y();

	// in an initializer, behind a statement trailer
	const a = fn()// c8
	.bar.baz; // c9
</script>
