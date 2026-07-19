<script lang="ts">
	// Same-line comment after `export =`, value authored on the next line: tsv
	// reflows the value inline, uniform with `export default` and every other
	// value-position gap. The comment keeps its authored position after the `=`.
	const value = { a: 1 };
	export = /* c */ value;
</script>
