<script lang="ts">
	// A `;` inside a trailing comment is comment text, not the statement's own
	// terminating semicolon.
	function fn1() {
		return a + b /* ; */ /* mid */;
	}

	function fn2() {
		throw a + b /* x; */;
	}
</script>
