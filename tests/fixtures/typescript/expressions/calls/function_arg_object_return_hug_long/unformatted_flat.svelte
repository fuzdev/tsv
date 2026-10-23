<script lang="ts">
	// At 100 the call fits on one line up to the body's `{`, so it stays flat
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});

	// At 101 it does not: the callback keeps the hug and its object return type breaks, since
	// the hug is measured only to that type's `{`
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});

	// At 100 the hugged line up to the return type's `{` fits, so the callback keeps the hug
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});

	// At 101 it does not, so the arguments break out
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});

	// a member callee
	o.fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});

	// a curried callee and a non-null one
	fn(a)(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});
	fn!(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});

	// a mapped return type
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { [K in X]: 1 } {
		g(a);
	});

	// a parameter that is not an object type
	fn(lead, function (a: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE): { x: X } {
		g(a);
	});

	// two preceding arguments, a named callback, a type parameter
	fn(lead, lead2, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});
	fn(lead, function named(a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});
	fn(lead, function <T>(a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});

	// a two-call member chain stays flat for the hug
	const v = o.a().fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});

	// the same chain as a call argument
	outer(o.a().fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	}));

	// an empty body
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {});

	// control: an empty object return type has nothing to break, so the arguments break out
	fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): {} {
		g(a);
	});

	// control: a lone callback hugs in both formatters
	fn(function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	});

	// control: an arrow callback breaks the arguments out in both formatters
	fn(lead, (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } => {
		g(a);
	});
</script>

<!-- a template expression -->
<p>
	{fn(lead, function (a: { b: C; d: EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE }): { x: X } {
		g(a);
	})}
</p>
