<script lang="ts">
	// the `= {` line at the print width keeps the delimiter on the `=` line; one column more
	// breaks after the `=`, as the comment-free authoring does
	type VeryLongTypeNameAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA = {
		a: B;
		// c1
	}[keyof B];

	type VeryLongTypeNameAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA = {
		a: B;
		// c2
	}[keyof B];
</script>
