<script lang="ts">
	interface I {
		// 100 chars - the type parameters fit, only the parameters break
		mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm<TFirst, TSecond extends Record<string, TFirstttttttttttt>>(
			input: TSecond
		): R;

		// 101 chars - the type parameters break one per line, and the parameters break with them
		mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm<
			TFirst,
			TSecond extends Record<string, TFirsttttttttttttt>
		>(
			input: TSecond
		): R;

		// a call signature takes the same layout
		<TFirst, TSecond extends Record<string, TFirstttttttttttttttttttttttttttttttttttttttttttttttt>>(
			input: TSecond
		): R;
		<
			TFirst,
			TSecond extends Record<string, TFirsttttttttttttttttttttttttttttttttttttttttttttttttt>
		>(
			input: TSecond
		): R;

		// a construct signature takes the same layout
		new <TFirst, TSecond extends Record<string, TFirstttttttttttttttttttttttttttttttttttttttttttt>>(
			input: TSecond
		): R;
		new <
			TFirst,
			TSecond extends Record<string, TFirsttttttttttttttttttttttttttttttttttttttttttttt>
		>(
			input: TSecond
		): R;

		// a type parameter list with no nested break point breaks the same way
		mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm<TFirst, TSecond extends Cccc>(
			input: TSecond
		): R;
		mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm<
			TFirst,
			TSecond extends Cccc
		>(
			input: TSecond
		): R;
	}

	// a type literal method takes the same layout
	type T = {
		mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm<TFirst, TSecond extends Record<string, TFirstttttttttttt>>(
			input: TSecond
		): R;
		mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm<
			TFirst,
			TSecond extends Record<string, TFirsttttttttttttt>
		>(
			input: TSecond
		): R;
	};

	// a function type takes the same layout
	type F = <
		TFirst,
		TSecond extends Record<string, TFirsttttttttttttttttttttttttttttttttttttttttttttttt>
	>(
		input: TSecond
	) => R;
</script>
