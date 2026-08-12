<script lang="ts">
	// A comment before the `!` of a sealed parenthesized optional chain stays inside
	// the parens, where the author wrote it, in every position the parens are
	// required: a `new` callee and a template tag.
	new (a?.b /* c */)!();
	new (a?.() /* c */)!();
	(a?.b /* c */)!`tpl`;
	(a?.() /* c */)!`tpl`;

	// The member-access position reaches the same gap through the chain printer.
	const a = (a?.b /* c */)!.ccc;
</script>
