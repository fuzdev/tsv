<script lang="ts">
	// A comment dangling in an EMPTY parameter list keeps its own line and the hug, whatever
	// the arrow's body — prettier's bailout never reads an empty list: a call body, after a
	// preceding argument
	fn(x, (
		// c1
	) => call());

	// a lone argument
	fn((
		// c2
	) => call());

	// a conditional body
	fn(x, (
		// c3
	) => (a ? b : c));

	// a lone argument
	fn((
		// c4
	) => (a ? b : c));

	// an object body
	fn(x, (
		// c5
	) => ({ a }));

	// an array body
	fn(x, (
		// c6
	) => [a]);

	// new
	new Fn(x, (
		// c7
	) => call());

	// member chain
	obj.a.fn(x, (
		// c8
	) => call());

	// a lone argument of a member chain
	obj.a.fn((
		// c9
	) => (a ? b : c));

	// a curried callee
	fn(a)(x, (
		// c10
	) => call());

	// an async arrow
	fn(x, async (
		// c11
	) => call());

	// a return type
	fn(x, (
		// c12
	): T => call());
</script>
