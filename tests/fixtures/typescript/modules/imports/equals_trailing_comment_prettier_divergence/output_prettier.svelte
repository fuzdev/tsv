<script lang="ts">
	// block comment after require() stays in place
	import x = require('./a') /* c */;

	// line comment after require() stays where the user placed it
	import y = require('./b'); // c

	// block comment after an entity-name reference stays in place
	import z = A.B /* d */;
</script>
