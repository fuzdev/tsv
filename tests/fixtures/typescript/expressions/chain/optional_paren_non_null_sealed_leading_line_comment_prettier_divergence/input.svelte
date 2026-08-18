<script lang="ts">
	// A `//` in a sealed shell's leading gap stays inside the parens and takes the
	// expanded shell — the comment on the `(` line, the chain indented, the `)!`
	// back out — in every position the parens are required: a `new` callee and a
	// template tag.
	new ( // c1
		a?.b
	)!();
	( // c2
		a?.b
	)!`tpl`;

	// An own-line comment keeps its own line inside the shell.
	new (
		// c3
		a?.b
	)!();

	// An inline block run leads the chain flat, matching prettier.
	new (/* c4 */ /* c5 */ a?.b)!();
	(/* c6 */ a?.())!`tpl`;

	// The member-access position reaches the same gap through the chain printer —
	// the sealed parenthesized base — and takes the same shell, with the assignment
	// hugging it (`= (`). The base's pair is required with or without the `!`.
	const m = ( // c7
		a?.b
	)!.ccc;
	const n = (
		// c8
		a?.b
	).ddd;

	// An inline block run leads the base flat here too.
	const o = (/* c9 */ a?.b)!.ccc;

	// The template tag is required with or without the `!` as well, and takes the
	// same shell — `(a?.b)`tpl`` seals the chain on its own.
	( // c10
		a?.b
	)`tpl`;
</script>
