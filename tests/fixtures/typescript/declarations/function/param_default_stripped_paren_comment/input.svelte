<script lang="ts">
	// a block comment inside a stripped paren shell trails the parameter it sits in
	function fn1(a1 = 1 /* c */) {}

	// a non-last parameter, where the comma ends the run instead of the closer
	function fn2(a2 = 1 /* c */, b2) {}

	// a line comment in the shell defers and breaks the list
	function fn3(
		a3 = 1 // c
	) {}
</script>
