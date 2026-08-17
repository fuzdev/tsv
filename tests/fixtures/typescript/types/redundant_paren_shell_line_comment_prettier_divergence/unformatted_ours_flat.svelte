<script lang="ts">
	// Cast keyword→value: the shell holds a leading run and a trailing one, and a third
	// comment follows it in the declarator's own gap
	const a = b as (
	// c1
	C // c2
	); // c3

	// The same position with the trailing run alone — the trailing `//` retains the
	// shell on its own, with no leading run to hang
	const d = e as (
	F // c4
	); // c5

	// Type annotation
	let g: (
	// c6
	H // c7
	); // c8

	// Type-alias `=` RHS
	type I = (
	// c9
	J // c10
	); // c11

	// Type-parameter `=` default
	type K<L = (
	// c12
	M // c13
	) // c14
	> = L;

	// Mapped-type value
	type N = {
		[O in string]: (
		// c15
		P // c16
		); // c17
	};

	// Type-predicate `is`
	function fn(q: unknown): q is (
	// c18
	R // c19
	) // c20
	{
		return true;
	}

	// Conditional-type `extends`
	type S = T extends (
	// c21
	U extends V ? W : X // c22
	) // c23
	? Y
	: Z;

	// Control: a leading run with no trailing one still hangs, the shell stripping and
	// the comment relocating into the keyword→value gap — the retention is keyed on the
	// TRAILING gap
	const a1 = b1 as (// c24
	C1);
</script>
