<script lang="ts">
	interface A {
		/** JSDoc for a */
		a :  string;
		/** JSDoc for b */
		b :  string;

		/** JSDoc for c */
		c :  string;
	}

	interface B {
		/**
		 * Multiline JSDoc
		 * with more lines
		 */
		value :  number;
	}

	interface C {
		/** JSDoc for d */  d :  string;
		/**
		 * Multiline JSDoc
		 */  e :  string;
	}
</script>
