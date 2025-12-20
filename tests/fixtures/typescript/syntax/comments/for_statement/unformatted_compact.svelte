<script lang="ts">
	// Comment in for-of body preserved
	for (x of y) {// comment
		fn();
	}

	// Comment in for header
	for (let i = 0;
// comment
		i < n; i++) {
		fn();
	}
</script>
