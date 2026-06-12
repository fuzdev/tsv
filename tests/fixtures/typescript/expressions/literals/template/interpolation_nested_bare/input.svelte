<script lang="ts">
	// Bare nested template with multiline content — TemplateLiteral hugs at ${}
	const a = `outer ${`inner
multiline
content`} rest`;

	// Short nested template — stays inline
	const b = `outer ${`short`} rest`;

	// Nested template with its own interpolation
	const c = `prefix ${`nested ${value} suffix`} end`;

	// Deeply nested templates
	const d = `a ${`b ${`c ${x} d`} e`} f`;

	// Tagged template inside interpolation — non-qualifying, hugs
	const e = `prefix ${tag`inner ${x} content`} end`;
</script>
