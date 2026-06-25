<script lang="ts">
	// Comment inside a computed generator-method key is preserved once (not duplicated onto the `*`)
	const o = {
		*[/* c1 */ a]() {},
		async *[/* c2 */ b]() {}
	};
</script>
