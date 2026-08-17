<script lang="ts">
	// A block comment before the comma trails its type parameter
	const fn1 = <T   /* c */ ,   U>(a: T,   b: U)  =>  a;

	// After the comma it leads the next one
	const fn2 = <T ,   /* c */   U>(a: T,   b: U)  =>  a;

	// Leading the first parameter, glued to the opening angle
	const fn3 = </* c */   T,   U>(a: T,   b: U)  =>  a;

	// Trailing the last parameter, before the closing angle
	const fn4 = <T,   U   /* c */>(a: T,   b: U)  =>  a;

	// A test call's callback inlines its type parameters at any width
	it('a',   <T   /* c */ ,   U>(a: T,   b: U)  =>  a);

	// A line comment expands the list
	const fn5 = <
		T,   // c
		U
	>(
		a: T,
		b: U
	)  =>  a;
</script>
