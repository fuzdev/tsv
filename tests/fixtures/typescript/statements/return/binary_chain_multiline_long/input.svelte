<script lang="ts">
	// Binary + chain with multi-line callback - parens wrap the return expression
	function fn1() {
		return (
			content +
			items
				.map((x) => {
					return x;
				})
				.join('')
		);
	}

	// Binary + chain with multi-line object literal - parens wrap
	function fn2() {
		return (
			content +
			items
				.map((x) => ({
					a: x,
				}))
				.join('')
		);
	}

	// 100 chars - chain callback fits on line, binary doesn't need parens
	function fn3(): string {
		return a + b + c + d + e + f + g + h + i + j + k + l + m + n + o + p + q + r + s + t + u + vvvv;
	}

	// 101 chars - binary breaks with parens (no multi-line content, pure width)
	function fn4(): string {
		return (
			a + b + c + d + e + f + g + h + i + j + k + l + m + n + o + p + q + r + s + t + u + vvvvv
		);
	}

	// Short binary + multi-line callback - still gets parens (hardlines force break)
	function fn5() {
		return (
			a +
			b.map((x) => {
				return x;
			})
		);
	}
</script>
