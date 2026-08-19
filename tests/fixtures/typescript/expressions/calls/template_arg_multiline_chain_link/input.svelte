<script lang="ts">
	// A multiline template as the sole argument of a call INSIDE a member chain.
	// The hug lives at the top of prettier's `printCallExpression`, above the
	// `printMemberChain` redirect — so a call the redirect swallowed never asks it.

	// Swallowed link: the redirect fires on `.c`, so this expands
	a.b(
		`line 1
line 2
`
	).c();

	// The same link with more of a chain after it
	a.b(
		`line 1
line 2
`
	)
		.c()
		.d(z);

	// Through a non-null and through `?.` — both still reach the redirect
	a.b(
		`line 1
line 2
`
	)!.c();
	a.b(
		`line 1
line 2
`
	)?.c();

	// A plain callee is where the chain walk STOPS, so this call keeps its own
	// layout and hugs
	a(`line 1
line 2
`).b();
	this.b(
		`line 1
line 2
`
	).c();

	// No call above it at all — nothing enters the chain, so it hugs
	a.b(`line 1
line 2
`).p;

	// The call above has a CALL callee, not a memberish one, so no redirect fires
	a.b(`line 1
line 2
`)({ p: 1 });
	template(`line 1
line 2
`)({ p: 1 });

	// The call above has its own sole multiline template, which preempts the
	// redirect before it can swallow anything
	a.b(`line 1
line 2
`).c(`line 3
line 4
`);
</script>
