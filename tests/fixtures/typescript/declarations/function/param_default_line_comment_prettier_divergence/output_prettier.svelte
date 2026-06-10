<script lang="ts">
	// Line comment after a parameter's `=` default stays after `=`.
	function fn(
		p = v, // c
	) {}

	// Same with a type annotation; the union stays inline.
	function fn2(
		p: A | B = v, // c
	) {}
</script>
