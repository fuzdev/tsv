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
</script>
