<script lang="ts">
	// A pure non-object intersection that breaks with its first member on its own
	// line (a nested "wrap-in-group" position) indents continuation members one
	// level deeper — unlike a bare type-alias RHS, where the first member shares
	// the `=` line and continuations align with it.

	// As a generic type argument
	type Arg = Partial<
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &
			Cccccccccccccccccccccccccccccc &
			Dddddddddddddddddddddddddddddd
	>;

	// As a tuple element
	type Tuple = [
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &
			Cccccccccccccccccccccccccccccc &
			Dddddddddddddddddddddddddddddd,
		X
	];

	// As a conditional-type branch
	type Cond = C extends D
		? Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
				Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &
				Cccccccccccccccccccccccccccccc &
				Dddddddddddddddddddddddddddddd
		: E;
</script>
