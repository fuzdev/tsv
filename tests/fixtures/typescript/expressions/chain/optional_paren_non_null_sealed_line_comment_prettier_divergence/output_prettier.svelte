<script lang="ts">
	// A line comment before the `!` of a sealed parenthesized optional chain forces
	// the multiline paren layout, keeping the comment inside the parens.
	new (a?.b)! // c
	();
	const t = (a?.b)! // c
	`tpl`;
	// The member-access position reaches the same gap through the chain printer;
	// the assignment hugs the shell (`= (`), which breaks at the comment.
	const m =
		(a
			?.b)! // c
		.ccc;
	// A multi-line block the base OWNS, in the same pair, with the `//` still in the
	// trailing gap: the pair prints its BROKEN body here, a second rendering of the same
	// operand, and the comment survives in it.
	const n =
		(/* a
b */ a
			?.b)! // c
		.ccc;
</script>
