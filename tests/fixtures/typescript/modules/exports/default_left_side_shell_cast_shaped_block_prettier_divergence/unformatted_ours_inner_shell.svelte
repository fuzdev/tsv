<script lang="ts">
	// JSDoc cast text inside the exported value's LEFTMOST shell, glued to a name rather than a
	// `(`, is no cast: an ordinary block that ends its line, so the value hangs under
	// `export default` as it does for any run that ends its line there.
	export default (/** @type {T} */
		f)(() => {
		q();
	});
</script>
