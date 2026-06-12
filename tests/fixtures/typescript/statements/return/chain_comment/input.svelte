<script lang="ts">
	// return with line comment in chain - wraps in parens
	function fn1(): string {
		return (
			a
				.b()
				// comment
				.c()
				.d()
		);
	}

	// throw with line comment in chain - wraps in parens
	function fn2() {
		throw (
			a
				.b()
				// comment
				.c()
		);
	}

	// return chain without comment - no parens
	function fn3(): string {
		return a.b().c().d();
	}
</script>
