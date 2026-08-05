<script>
	// expand-first shape whose cast second arg holds a source-multiline object:
	// all args break out, in the plain and member-chain printers alike
	fn(
		() => {
			tick();
		},
		/** @type {T} */ ({
			a: 1
		})
	);

	obj.aa(x).bb(
		() => {
			tick();
		},
		/** @type {T} */ ({
			a: 1
		})
	);
</script>
