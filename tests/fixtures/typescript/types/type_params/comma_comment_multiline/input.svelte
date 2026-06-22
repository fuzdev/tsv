<script lang="ts">
	// Type param declaration: inline block comment after comma
	function fn1<
		T,
		// line comment
		/* c */ U
	>() {}

	// Type param declaration: trailing block comment before comma
	function fn2<
		T /* c */,
		// line comment
		U
	>() {}

	// Type param instantiation (call): inline block comment after comma
	fn1<
		string,
		// line comment
		/* c */ number
	>();

	// Type param instantiation (call): trailing block comment before comma
	fn1<
		string /* c */,
		// line comment
		number
	>();
</script>
