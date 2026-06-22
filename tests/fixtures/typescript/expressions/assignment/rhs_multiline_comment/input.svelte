<script>
	// multiline block comment before arrow in object property - forces break after colon
	const x = {
		a:
			/**
			 * comment
			 * @param {string} x
			 */
			(x) => {
				return x;
			}
	};

	// multiline block comment before value in variable declaration
	const b =
		/**
		 * comment
		 */
		(x) => {
			return x;
		};

	// multiline block comment before non-arrow value in object property
	const y = {
		a:
			/**
			 * comment
			 */
			fn()
	};

	// inline (single-line) block comment stays inline - no break
	const z = {
		a: /** inline */ fn()
	};

	// inline block comment on variable declaration stays inline
	const c = /** inline */ fn();
</script>
