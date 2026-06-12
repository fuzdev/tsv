<script lang="ts">
	// Multiple type args in return type - breaks between args at 101+ chars

	// 100 chars - stays inline
	function fn100(): Result<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA, B> {}

	// 101 chars - breaks between type args
	function fn101(): Result<
		AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA,
		B
	> {}

	// Arrow 100 chars - stays inline
	const arrow100 = (): Result<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA, B> => {};

	// Arrow 101 chars - breaks between type args
	const arrow101 = (): Result<
		AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA,
		B
	> => {};

	// Class method 100 chars - stays inline
	class C {
		method100(): Result<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA, B> {}

		// Class method 101 chars - breaks between type args
		method101(): Result<
			AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA,
			B
		> {}
	}
</script>
