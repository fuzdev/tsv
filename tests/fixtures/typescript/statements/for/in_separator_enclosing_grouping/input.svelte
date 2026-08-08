<script>
	// The `in` of a for-in binds as the loop separator, not a binary operator, when
	// the loop sits inside grouping delimiters that belong to an outer expression.
	fn(function () {
		for (key in obj) {
			expr;
		}
	});

	const a = {
		m() {
			for (key in obj) {
				expr;
			}
		}
	};

	const b = [
		() => {
			for (key.prop in obj) {
				expr;
			}
		}
	];

	// Unaffected baselines: the declaration form, and a header at the top level.
	fn(function () {
		for (const key1 in obj) {
			expr;
		}
	});

	for (key in obj) {
		expr;
	}

	// A grouping inside the header itself still restores `in` as a binary operator.
	fn(function () {
		for (c = (key in obj); d; e) {
			expr;
		}
	});
</script>
