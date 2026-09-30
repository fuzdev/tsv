<script lang="ts">
	// A parameter default ending in a division, ahead of a regex holding a `)`: the
	// comment stays inside the parameter list, where the author wrote it.

	// After a type-argument list's `>`
	function fn1(a = f<T> / 2, b = /[)]/ /* c */) {}

	// After a postfix non-null `!`
	function fn2(a = c! / 2, b = /[)]/ /* c */) {}

	// A prefix `!` after a statement's `)` opens the regex itself
	function fn3(
		a = () => {
			if (c)!/[)]/.test(d);
		},
		b /* c */
	) {}
</script>
