<script lang="ts">
	// A multi-line block comment in a value seam's gap binds to the UNION, not to its
	// first member, so prettier prints it outside the union's group: a union that fits
	// stays flat below the comment and gains no leading `|`. Both multi-line kinds ask
	// the same question, and every seam that hands its gap run into the union answers it
	// the same way — the cells below are a census of those seams, and the two pipe
	// variants carry every one of them.

	// annotation `:`
	let v1:
		/* c
d */ | Ooo | Ppp;

	// annotation `:`, indentable kind
	let v2:
		/**
		 * c
		 */ | Ooo | Ppp;

	// return type
	function fn():
		/* c
d */ | Ooo | Ppp {
		return null as never;
	}

	// type argument `<`
	type T1 = Box<
		/* c
d */ | Ooo | Ppp
	>;

	// a paren the printer strips
	type T2 = (
		/* c
d */ | Ooo | Ppp
	)[];

	// conditional `extends`
	type T3<X> = X extends
		/* c
d */ | Ooo | Ppp
		? 1
		: 2;

	// property signature `:`
	type T4 = {
		k:
			/* c
d */ | Ooo | Ppp;
	};

	// `as`
	const a1 = v1 as
		/* c
d */ | Ooo | Ppp;

	// `satisfies`
	const a2 = v1 satisfies
		/* c
d */ | Ooo | Ppp;

	// type alias `=`
	type T5 =
		/* c
d */ | Ooo | Ppp;

	// function-type return `=>`
	type T6 = () =>
		/* c
d */ | Ooo | Ppp;

	// index signature value
	type T7 = {
		[k: string]:
			/* c
d */ | Ooo | Ppp;
	};

	// mapped-type value
	type T8 = {
		[K in string]:
			/* c
d */ | Ooo | Ppp;
	};

	// tuple member
	type T9 = [
		/* c
d */ | Ooo | Ppp
	];

	// type-parameter constraint
	type T10<
		X extends
			/* c
d */ | Ooo | Ppp
	> = X;

	// type-parameter default
	type T11<
		X =
			/* c
d */ | Ooo | Ppp
	> = X;

	// null control: a single-line block binds to the first member instead
	type T12 = /* c */ | Ooo | Ppp;

	// null control: an intersection is unaffected
	type T13 = /* c
d */ Ooo & Ppp;

	// null control: no comment
	type T14 = Ooo | Ppp;

	export type { T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14 };
	export { v1, v2, fn, a1, a2 };
</script>
