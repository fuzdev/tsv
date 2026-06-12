<script lang="ts">
	// Line comments with blank line between (class body)
	class A {
		a = 1;

		// section

		// section2
		b = 2;
	}

	// Line comments with blank line between (function body)
	function fn() {
		a();

		// section

		// section2
		b();
	}

	// Line comment + blank + JSDoc (class body)
	class B {
		a = 1;

		// Private

		/**
		 * JSDoc.
		 */
		b = 2;
	}

	// Line comment + blank + JSDoc (function body)
	function fn2() {
		a();

		// Private

		/**
		 * JSDoc.
		 */
		b();
	}
</script>
