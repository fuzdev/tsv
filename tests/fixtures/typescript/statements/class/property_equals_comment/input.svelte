<script lang="ts">
	class A {
		// Basic property
		b /* c1 */ = 1;

		// With type annotation
		c: number /* c2 */ = 1;

		// Static property
		static d /* c3 */ = 1;

		// Optional with type
		f?: number /* c5 */ = 1;

		// Readonly property
		readonly g /* c6 */ = 1;
	}
</script>
