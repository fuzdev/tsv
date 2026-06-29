<script>
	// Same-line comment after `export default`, value authored on the next line:
	// tsv preserves the comment trailing the keyword and the value's break. The
	// value indents one level (the uniform module-header indent); prettier keeps
	// it at the statement's own indent. A value glued to the comment stays inline.
	export default /* c */
		x;
</script>
