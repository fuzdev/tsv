<script>
	function fn1() {
		function fn2() {
			// Ternary with binaryish test: comment causes line to exceed width,
			// forces break-after-operator layout
			const a = {
				prop:
					/* @type {string | ArrayBufferView} */ input instanceof Request && cloned_body
						? fn(a)
						: b,
			};

			// Without comment, same ternary: fits inline
			const c = {
				prop: input instanceof Request && cloned_body ? fn(a) : b,
			};

			// Without comment, same ternary exceeding width: also breaks after colon
			const d = {
				prop:
					input_abcd instanceof Request && cloned_body_abcdefghijklmnopqrstuvwxyzabcdefghijkl
						? fn(a)
						: b,
			};
		}
	}
</script>
