<script lang="ts">
	// a lone same-line comment after the call collapses, trailing past the member
	fn().bar; // c1

	// an own-line comment before the trailing member breaks the chain and keeps its line
	// c2
	fn().bar;

	// a same-line comment then an own-line one: the chain breaks, each comment in place
	// c4
	fn() // c3
	.bar;

	// the same rule through an optional member
	// c5
	fn()?.bar;

	// and past a non-null on the call, in an initializer
	const a =
		// c6
		fn()!.bar;
</script>
