<script>
	function fn1() {
		function fn2() {
			// Inline block comment + logical OR: assignment layout with non-ternary
			const a = {
				prop: /* c */ very_long_variable_name || another_long_variable_name || final_fallback_value,
			};

			// Without comment: same expression fits inline
			const b2 = {
				prop: very_long_variable_name || another_long_variable_name || final_fallback_value,
			};

			// Post-colon block comment + nullish coalescing
			const c = {
				prop: /* c */ very_long_variable_abcdef ?? another_long_variable_abcdef ?? fallback_abcdefg,
			};
		}
	}
</script>
