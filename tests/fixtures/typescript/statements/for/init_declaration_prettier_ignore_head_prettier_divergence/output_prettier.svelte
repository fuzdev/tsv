<script lang="ts">
	// a `for` init that is a declaration freezes over the declaration only; the header's
	// own `;` stays parent-owned
	for (
		// prettier-ignore
		let i  =  0, j = 1;;
		i < 10;
		i++
	) {
		fn();
	}
</script>
