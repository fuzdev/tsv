<script>
	// block comment between export and braces (re-export)
	export {} from /* c1 */ './a';

	// line comment between export and braces (re-export)
	export {} from // c2
	'./b';
</script>
