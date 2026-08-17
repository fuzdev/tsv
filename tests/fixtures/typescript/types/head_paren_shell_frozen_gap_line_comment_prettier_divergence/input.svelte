<script lang="ts">
	// a type-argument list's FIRST argument, frozen: the shell rides out inside the
	// verbatim slice, so the list's own leading run stops at the directive
	type A = B<
		// prettier-ignore
		( // c1
			C)[]
	>;

	// a later argument in the same list
	type D = E<
		F,
		// prettier-ignore
		( // c2
			G)[]
	>;

	// a conditional's `?` branch gap
	type H = I extends J
		// prettier-ignore
		? ( // c3
				K)[]
		: L;

	// its `:` branch twin
	type M = N extends O
		? P
		// prettier-ignore
		: ( // c4
				Q)[];

	// a function type's `=>` return gap
	type R = () =>
		// prettier-ignore
		( // c5
			S)[];

	// a type-alias `=` RHS whose shell is an INTERSECTION's first member — the freeze
	// slices that member, hoist and all
	type T =
		// prettier-ignore
		( // c6
			U) &
			V;

	// control: a tuple element's list gap
	type W = [
		// prettier-ignore
		( // c7
			X)[]
	];

	// control: a union member's `|` gap
	type Y =
		| Z
		// prettier-ignore
		| ( // c8
			A1)[];
</script>
