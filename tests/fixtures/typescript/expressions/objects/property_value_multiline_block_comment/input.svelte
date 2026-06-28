<script lang="ts">
	const o = {
		// Multiline block comment before value: breaks `:`, value stays glued to `*/`
		foo:
			/*
			 * c
			 */ bar,
		// Single-line block comment: stays inline
		baz: /* c */ qux,
		// Single-line block comment on its own line: stays on its own line
		quux:
			/* own line */
			val
	};
</script>
