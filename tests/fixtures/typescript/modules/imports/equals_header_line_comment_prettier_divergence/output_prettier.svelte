<script lang="ts">
	// A line comment in each gap of the import-equals header — each stays where the
	// author wrote it, and the whole header continues at ONE indent level
	export // c1
	import // c2
	A = require('./a'); // c3
	// c4

	// The module reference continues one level down, and its own breaks nest inside
	// that level — its contents one deeper than `require(`, its `)` back at the
	// reference's own level
	import B = require( // c5
		// c6
		'./b' // c7
		// c8
	); // c9
</script>
