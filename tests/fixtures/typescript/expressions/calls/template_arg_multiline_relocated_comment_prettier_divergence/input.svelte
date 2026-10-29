<script lang="ts">
	// A comment on a line of its own ahead of a sole multiline template argument — moved in
	// from between the callee and its `(`, or written inside the parens — keeps that line,
	// so the template starts a line of its own and the arguments break
	aaa()(
		/* c1 */
		`
`
	);
	new Bbb(
		/* c2 */
		`
`
	);
	ccc.ddd(
		/* c3 */
		`
`
	);
	await import(
		/* c7 */
		`a
b`
	);

	// A block glued to the callee is glued to the template, which hugs the `(` — at a chain
	// link too
	[](/* c4 */ `
`).eee();

	// A block glued to a stripped `(` stays glued to the template, which hugs the `(`
	fff(/* c5 */ `a
b`);
	await import(/* c8 */ `a
b`);
	// … where the chain link's own layout is decided by the same question
	x = [](/** c9 */ `a
b`)
		.b()
		.c();

	// The outer call's hug decides whether the chain swallows the link below it: here the
	// outer template takes its own line, so the link expands as a member-chain link does
	www
		.yyy(
			`p
q`
		)
		.zzz(
			/* c10 */
			`
`
		);
</script>

<p>
	{ggg(
		/* c6 */
		`
`
	)}
</p>
