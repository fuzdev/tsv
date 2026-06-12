<script lang="ts">
	// Block comment between function name and type params
	function fn1 /* c1 */ <T>(x: T): T {
		return x;
	}

	// Function expression - block comment between name and type params
	const a = function fn2 /* c2 */ <T>(x: T): T {
		return x;
	};
</script>
