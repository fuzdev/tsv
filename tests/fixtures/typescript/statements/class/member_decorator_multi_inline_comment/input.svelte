<script lang="ts">
	class A {
		// two inline block comments before a field stay on one line
		@fn
		/* c1 */ /* c2 */
		a = 1;

		// before a method
		@fn
		/* c3 */ /* c4 */
		method() {}

		// before an accessor
		@fn
		/* c5 */ /* c6 */
		accessor x = 1;
	}
</script>
