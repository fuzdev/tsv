<script lang="ts">
	// A comment on its own line in the operand→`!` gap of a sealed parenthesized
	// optional chain keeps its own line inside the parens, below the operand, in
	// authored order (a block following a `// c` is always own-line — the `//`
	// owns the rest of its line). Same rule at every sealed position: a `new`
	// callee, a template tag, a member access.
	new (a?.() // c
	/* d */)!();
	const t = (a?.b // c
	/* d */)!`tpl`;
	const m = (a?.b // c
	/* d */)!.ccc;
	new (a?.b
	// c
	)!();
</script>
