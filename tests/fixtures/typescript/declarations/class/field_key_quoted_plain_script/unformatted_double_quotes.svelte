<script>
	// A script with no `lang` keeps a quoted class field key quoted, the same as a TypeScript one.
	// A JSDoc-typed field with no initializer is the form a type-checked script treats differently
	// by spelling.
	class A {
		"a" = 1;
		"b";
		static "c" = 2;
		"fn" = () => 3;
		/** @type {string} */
		"d";
		'm'() {}
	}
</script>
