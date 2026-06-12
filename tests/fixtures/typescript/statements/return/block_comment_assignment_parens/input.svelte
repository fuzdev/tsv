<script>
	// return with block comment before assignment parens
	function fn1() {
		return /* comment */ (a = b);
	}

	// return with block comment before nullish assignment parens
	function fn2() {
		return /* comment */ (a ??= b);
	}

	// return with block comment before assignment, multiline object
	function fn3() {
		return /* comment */ (a ??= {
			b: c(),
			d: e(),
		});
	}

	// throw with assignment parens (no comment)
	function fn4() {
		throw (a = b);
	}

	// throw with block comment before assignment parens
	function fn5() {
		throw /* comment */ (a = b);
	}

	// throw with block comment before assignment, multiline object
	function fn6() {
		throw /* comment */ (a ??= {
			b: c(),
			d: e(),
		});
	}
</script>
