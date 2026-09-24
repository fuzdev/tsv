<script>
	// a parenthesized simple target is a DestructuringAssignmentTarget, so an ASSIGNMENT
	// pattern takes one in every element position; the parens are grouping and drop
	[(a1)] = x;
	[...(a2)] = x;
	({ k: (a3) } = x);
	({ ...(a4) } = x);
	[((a5.b))] = x;
	[(a6) = 1] = x;
	({
		k: [(a7)]
	} = x);
	for ([(a8)] of xs);
	for ([(a9)] in o);
	() => [(a10)] = x;

	// parens in a binding element initializer are expression grouping, not a target
	let [a11 = (1)] = x;
	function f1([a12 = (1)]) {}

	// an assignment pattern INSIDE a binding default is still an assignment
	function f2(a13 = ([(b13)] = x)) {}
	([a14 = ([(b14)] = x)]) => 1;
	let { a15 = [(b15)] = x } = y;
</script>
