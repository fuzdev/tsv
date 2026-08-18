<script lang="ts">
	// The `export =` twin of `default_operand_paren_line_comment`: a `//` written inside
	// the exported value's own grouping parens stays inside them, rather than merging with
	// whatever already trails the statement's line.
	export = (
		x // c1
	); // c2
</script>
