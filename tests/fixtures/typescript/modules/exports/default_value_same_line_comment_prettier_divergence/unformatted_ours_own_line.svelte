<script>
	// Same-line comment after `export default`, value authored on the next line:
	// tsv reflows the value inline, uniform with `export =` and every other
	// value-position gap. The comment keeps its authored position after the
	// keyword; only the author's line break is reflowed.
	export default
		/* c */

		x;
</script>
