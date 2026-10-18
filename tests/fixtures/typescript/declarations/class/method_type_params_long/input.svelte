<script lang="ts">
	class C {
		// 100 chars - the type parameters fit, only the parameters break
		mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm<TFirst, TSecond extends Record<string, TFirstttttttttttt>>(
			input: TSecond
		): R {}

		// 101 chars - the type parameters break one per line, the parameters stay flat
		mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm<
			TFirst,
			TSecond extends Record<string, TFirsttttttttttttt>
		>(input: TSecond): R {}

		// a static method takes the same layout
		static mmmmmmmmmmmmmmmmmmmmmmmmmmmmm<TFirst, TSecond extends Record<string, TFirstttttttttttt>>(
			input: TSecond
		): R {}
		static mmmmmmmmmmmmmmmmmmmmmmmmmmmmm<
			TFirst,
			TSecond extends Record<string, TFirsttttttttttttt>
		>(input: TSecond): R {}

		// a type parameter list with no nested break point breaks the same way
		mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm<TFirst, TSecond extends Cccc>(
			input: TSecond
		): R {}
		mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm<
			TFirst,
			TSecond extends Cccc
		>(input: TSecond): R {}
	}

	// an object method takes the same layout
	const o = {
		mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm<
			TFirst,
			TSecond extends Record<string, TFirsttttttttttttt>
		>(input: TSecond): R {}
	};

	// a function declaration takes the same layout
	declare function mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm<
		TFirst,
		TSecond extends Record<string, TFirstttt>
	>(input: TSecond): R;
</script>
