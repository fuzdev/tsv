<script lang="ts">
	// Line comment after a parameter's `=` default stays after `=`.
	function fn(
		p = v // c
	) {}

	// Same with a type annotation; the union stays inline.
	function fn2(
		p: A | B = v // c
	) {}

	// A multi-line comment glued to the value below the line comment: the value keeps its own
	// layout.
	function fn3(
		/* d
		 */ p3 = aaa ? bbb : ccc // c
	) {}
</script>
