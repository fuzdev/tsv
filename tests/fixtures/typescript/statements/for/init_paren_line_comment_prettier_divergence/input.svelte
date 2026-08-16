<script lang="ts">
	// A line comment in the trailing gap of a for-header init declarator's grouping
	// shell. The shell is RETAINED and the comment stays inside it — deferring it
	// past the header's `;` would carry it out of the construct it was written in.
	for (
		let i = (
			a // c1
		);
		;
	) {
		fn();
	}

	// A run of two keeps both inside the same shell.
	for (
		let i = (
			a // c2
			// c3
		);
		;
	) {
		fn();
	}

	// Any declarator index, and a populated header.
	for (
		let i = 0,
			j = (
				b // c4
			);
		i < n;
		i++
	) {
		fn();
	}
</script>
