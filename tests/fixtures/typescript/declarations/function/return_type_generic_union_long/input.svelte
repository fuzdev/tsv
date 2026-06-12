<script lang="ts">
	// Union in return type generic - breaks at 101+ chars

	// 100 chars - stays inline
	function fn100(): Promise<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | B> {}

	// 101 chars - breaks inside <>
	function fn101(): Promise<
		AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | B
	> {}

	// Arrow 100 chars - stays inline
	const arrow100 = (): Promise<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | B> => {};

	// Arrow 101 chars - breaks inside <>
	const arrow101 = (): Promise<
		AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | B
	> => {};

	// Class method 100 chars - stays inline
	class C {
		method100(): Promise<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | B> {}

		// Class method 101 chars - breaks inside <>
		method101(): Promise<
			AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | B
		> {}
	}
</script>
