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
</script>
