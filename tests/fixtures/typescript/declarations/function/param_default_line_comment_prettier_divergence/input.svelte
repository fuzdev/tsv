<script lang="ts">
	// Line comment after a parameter's `=` default stays after `=`.
	function fn(
		p = // c
		v,
	) {}

	// Same with a type annotation; the union stays inline.
	function fn2(
		p: A | B = // c
		v,
	) {}
</script>
