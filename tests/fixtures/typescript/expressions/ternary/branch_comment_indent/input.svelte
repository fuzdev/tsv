<script lang="ts">
	// A leading own-line comment on the consequent must not drop an indent level
	// from the consequent's own multiline content (the broken object argument)
	const x = cond
		? // c
			fnnnnnnnnnnnnnnnnnnnnnn.methoddddddddddddddddddddd({
				prop1: aaaaaaaaaaaa,
				prop2: bbbbbbbbbbbb
			})
		: other;
</script>
