<script lang="ts">
	// an own-line directive between type parameters freezes only the following
	// parameter — function declaration
	function fn1<
		T,
		// prettier-ignore
		U extends {x:   1},
		V
	>(): void {}

	// before the first parameter (Rule A — first member like every member)
	function fn2<
		// prettier-ignore
		T extends {x:   1},
		U
	>(): void {}

	// single parameter
	function fn3<
		// prettier-ignore
		T extends {x:   1}
	>(): void {}

	// interface header
	interface A<
		// prettier-ignore
		T extends {x:   1},
		U
	> {
		a: T;
	}

	// class header
	class B<
		// prettier-ignore
		T extends {x:   1},
		U
	> {}

	// arrow function (two params)
	const fn4 = <
		// prettier-ignore
		T extends {x:   1},
		U
	>(
		a: T,
		b: U
	): void => {};
</script>
