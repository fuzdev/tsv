<script lang="ts">
	// parenthesized default values (sequence and assignment forms keep their parens)
	function fn1(a, b = (c, d)) {}
	const fn2 = (a = (b = c)) => a;
	const {e = (f, g)} = h;

	// parenthesized class property values
	class A {
		a = (this.a = b);
	}

	// parenthesized new callees followed by member access
	new (fn3().a)();
	new (fn3()[b])();
</script>
