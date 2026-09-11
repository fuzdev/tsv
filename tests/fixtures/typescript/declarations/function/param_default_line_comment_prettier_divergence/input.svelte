<script lang="ts">
	// Line comment after a parameter's `=` default stays after `=`.
	function fn(
		p = // c
			v
	) {}

	// Same with a type annotation; the union stays inline.
	function fn2(
		p: A | B = // c
			v
	) {}

	// A multi-line comment glued to the value below the line comment: the value keeps its own
	// layout.
	function fn3(
		p3 = // c
			/* d
			 */ aaa ? bbb : ccc
	) {}
</script>
