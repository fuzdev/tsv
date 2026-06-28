<script lang="ts">
	const o = {
		// Multiline block comment before value: breaks `:`, value stays glued to `*/`
		foo:
			/*
			 * c
			 */ bar,
		// Single-line block comment: stays inline
		baz: /* c */ qux
	};
</script>
