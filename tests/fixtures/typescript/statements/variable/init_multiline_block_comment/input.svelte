<script lang="ts">
	// Multiline block comment before init value: breaks `=`, value stays glued to `*/` (3.9 #19180)
	const foo =
		/*
		 * comment
		 */ bar;
	// Single-line block comment: stays inline (both formatters)
	const baz = /* comment */ qux;
</script>
