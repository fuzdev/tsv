<script lang="ts">
	// Multiline block comment before init value: breaks `=`, value stays glued to `*/`
	const foo =
		/*
		 * comment
		 */ bar;
	// Single-line block comment: stays inline
	const baz = /* comment */ qux;
</script>
