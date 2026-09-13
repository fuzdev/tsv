<script lang="ts">
	// The MULTI-LINE BLOCK spelling of
	// `operand_paren_leading_comment_kept_shell_prettier_divergence`: a `MultiLineComment`
	// holding a line terminator IS one for ASI (ECMA-262 §12.4), so the shell of a cast's or a
	// postfix update's operand is load-bearing here exactly as it is for a `//`. The pair
	// holds the comment, nothing reaches the keyword's line, and no hanging pair is added.
	function fn1() {
		return (
			/* a
b */ a
		) satisfies B;
	}
	function fn2() {
		return (
			/* a
b */ a
		)++;
	}
	function fn3() {
		return (
			/* a
b */ a
		) as B;
	}
	function fn4() {
		throw (
			/* a
b */ a
		) as B;
	}
	function* fn5() {
		yield (
			/* a
b */ a
		) as B;
	}

	// control: a single-line block spans no line, so nothing is load-bearing and the
	// redundant shell drops
	function fn6() {
		return /* c */ a as B;
	}
</script>
