<script lang="ts">
	// Comment inside a computed accessor key is preserved once (not duplicated onto the keyword)
	const o = {
		get [/* c1 */ a]() {
			return 1;
		},
		set [/* c2 */ b](v) {}
	};
</script>
