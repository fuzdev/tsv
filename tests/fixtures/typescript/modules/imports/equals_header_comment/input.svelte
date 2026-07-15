<script lang="ts">
	// Block comment between `import` and the name
	import /* c1 */ A = require('./a');

	// Block comment between the name and `=`
	import B /* c2 */ = require('./b');

	// Block comment between `=` and the module reference
	import C = /* c3 */ require('./c');

	// Block comment between the `type` keyword and the name
	import type /* c4 */ D = require('./d');

	// Block comment between `export` and `import`
	export /* c5 */ import E = require('./e');
</script>
