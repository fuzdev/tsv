<script lang="ts">
	// 100 chars at 1 tab - fits on one line
	const a = /aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/.exec(b);

	// Short control - fits easily
	const c = /short/.exec(b);

	// Deep indent - call head exceeds 100, breaks after =
	function fn() {
		function inner() {
			const d =
				/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/.exec(b);
		}
	}

	// Deep indent - call head fits (99), expands call args
	function fn2() {
		function inner2() {
			const e = /aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/.exec(
				b,
			);
		}
	}
</script>
