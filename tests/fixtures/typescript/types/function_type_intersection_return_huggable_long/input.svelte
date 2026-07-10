<script lang="ts">
	// Huggable object member in a function-type intersection return: the object owns
	// its own expansion — its body indents ONE level and `}` returns to base, NOT
	// double-indented from the `=>`. Mirrors the type-alias RHS layout.

	// Object LAST at exactly 100 chars — hugs `&`, stays inline
	type Fitxx = () => Aaaaaa & { propx: BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB };

	// Object LAST at 101 chars — breaks; body at base+1, `}` at base
	type Break = () => Aaaaaa & {
		propx: BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB;
	};

	// Object FIRST — expands, trailing `& A` stays on the `}` line
	type ExpandingFirst = () => {
		property_one: string;
		property_two: number;
		property_three: T;
	} & Abcde;

	// Constructor type shares the same return path (`new`)
	type Ctor = new () => Aaaaaa & {
		property_one: string;
		property_two: number;
		property_three: Ttttttt;
	};
</script>
