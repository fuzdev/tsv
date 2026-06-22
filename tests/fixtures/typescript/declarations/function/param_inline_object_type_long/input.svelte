<script lang="ts">
	// Function: 100 chars - stays inline
	function fn1(a?: { b?: (x: A) => A | null; c?: (x: A) => A | null; d?: numberABCDEFGHI }): void {}

	// Function: 101 chars - wraps: a?: { hugs function name, properties expand
	function fn2(a?: {
		b?: (x: A) => A | null;
		c?: (x: A) => A | null;
		d?: numberABCDEFGHIJ;
	}): void {}

	class C {
		// Method: 100 chars - stays inline
		m1(a?: { b?: (x: A) => A | null; c?: (x: A) => A | null; d?: numberABCDEFGHIJKLMNOPQ }): void {}

		// Method: 101 chars - wraps: a?: { hugs method name, properties expand
		m2(a?: {
			b?: (x: A) => A | null;
			c?: (x: A) => A | null;
			d?: numberABCDEFGHIJKLMNOPQR;
		}): void {}
	}
</script>
