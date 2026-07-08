<script lang="ts">
	class A {
		m1(/* c */
			x) {}

		m2(/* c */
			a, b) {}

		m3(
			// c
			x
		) {}

		m4(
			/* c */
			x
		) {}

		m5(
			a,
			/* c */
			b
		) {}
	}

	const f = (/* c */
		x) => {};
</script>
