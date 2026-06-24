<script lang="ts">
	// A `null`/`void` second union member in a generic return type breaks inside
	// the `<>` at the 101-char boundary, consistently across declaration kinds.

	// 100 chars - stays inline (both us and Prettier)
	function fn100(): Promise<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | null> {}

	// 101 chars - breaks inside `<>` (Prettier leaves the line over print width)
	function fn101(): Promise<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | null> {}

	// Arrow 100 chars - stays inline (both us and Prettier)
	const arrow100 = (): Promise<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | null> => {};

	// Arrow 101 chars - breaks inside `<>` (Prettier breaks at the assignment `=`)
	const arrow101 =
		(): Promise<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | null> => {};

	// Class method 100 chars - stays inline (both us and Prettier)
	class C {
		method100(): Promise<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | null> {}

		// Class method 101 chars - breaks inside `<>` (Prettier leaves it over print width)
		method101(): Promise<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | null> {}
	}
</script>
