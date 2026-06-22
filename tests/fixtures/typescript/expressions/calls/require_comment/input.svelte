<script lang="ts">
	// Comment inside require() on separate line (#18037)
	require(
		// comment
		'a'
	);

	// Inline comment
	require(/* comment */ 'b');

	// TypeScript import = require with comment (#18035)
	import c = require(
		// comment
		'c'
	);
</script>
