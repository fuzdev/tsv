<script lang="ts">
	function fn1() {
		// an own-line directive in the grouping `(`→operand gap freezes the whole operand
		return (
			// prettier-ignore
			a  +  b
		);
	}

	function fn2() {
		// `throw` shares the hanging-paren layout, so it shares the freeze
		throw (
			// prettier-ignore
			a  +  b
		);
	}

	function fn3() {
		// an own-line block comment behaves identically — placement keys the freeze
		return (
			/* prettier-ignore */
			a  +  b
		);
	}

	function fn4() {
		// a sequence operand freezes WHOLE: the directive leads the sequence node, so every
		// operand rides inside the verbatim slice
		return (
			// prettier-ignore
			a  (  1  ),
			b  (  2  )
		);
	}

	function fn5() {
		// an inter-operand directive inside that sequence freezes only the following operand
		return (
			a  (  1  ),
			// prettier-ignore
			b  (  2  )
		);
	}
</script>
