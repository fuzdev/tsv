<script lang="ts">
	// Prettier bug: doesn't break when second union member is `null` or `void`
	// We break consistently at 101+ chars regardless of type keyword

	// 100 chars - stays inline (both us and Prettier)
	function fn100(): Promise<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | null> {}

	// 101 chars - we break, Prettier exceeds print width
	function fn101(): Promise<
		AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | null
	> {}

	// Arrow 100 chars - stays inline (both us and Prettier)
	const arrow100 = (): Promise<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | null> => {};

	// Arrow 101 chars - we break inside <>, Prettier uses assignment break
	const arrow101 = (): Promise<
		AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | null
	> => {};

	// Class method 100 chars - stays inline (both us and Prettier)
	class C {
		method100(): Promise<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | null> {}

		// Class method 101 chars - we break, Prettier exceeds print width
		method101(): Promise<
			AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | null
		> {}
	}
</script>
