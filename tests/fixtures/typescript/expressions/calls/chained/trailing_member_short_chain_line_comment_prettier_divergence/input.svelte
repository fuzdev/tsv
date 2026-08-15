<script lang="ts">
	// a lone same-line comment after the call collapses, trailing past the member
	fn().bar; // c1

	// an own-line comment before the trailing member breaks the chain and keeps its line
	fn()
		// c2
		.bar;

	// a same-line comment then an own-line one: the chain breaks, each comment in place
	fn() // c3
		// c4
		.bar;

	// the same rule through an optional member
	fn()
		// c5
		?.bar;

	// and past a non-null on the call, in an initializer
	const a = fn()!
		// c6
		.bar;
</script>
