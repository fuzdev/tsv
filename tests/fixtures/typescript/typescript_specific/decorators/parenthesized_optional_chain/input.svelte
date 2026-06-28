<script lang="ts">
	class A {
		// a parenthesized optional-chain callee keeps an outer paren wrapping the
		// whole decorator expression
		@((a?.b)())
		x;
	}
</script>
