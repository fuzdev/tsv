<script lang="ts">
	// A line comment before the `!` of a sealed parenthesized optional chain forces
	// the multiline paren layout, keeping the comment inside the parens.
	new (a?.b)! // c
	();
	const t = (a?.b)! // c
	`tpl`;
</script>
