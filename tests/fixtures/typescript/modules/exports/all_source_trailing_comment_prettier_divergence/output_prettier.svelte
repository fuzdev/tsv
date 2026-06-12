<script>
	// block comment after the re-export source stays in place
	export * from './a' /* c */;

	// line comment after the re-export source stays where the user placed it
	export * as ns from './b'; // c
</script>
