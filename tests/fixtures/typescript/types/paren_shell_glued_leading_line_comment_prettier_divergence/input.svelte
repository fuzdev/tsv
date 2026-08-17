<script lang="ts">
	// A line comment the author glued to a paren shell's `(` keeps that line, as at
	// every other opening delimiter. Optional tuple element, union operand
	type A = [
		( // c1
			B | C
		)?
	];

	// The same element with an operand the pair is required for
	type D = [
		( // c2
			E extends F ? G : H
		)?
	];

	// A redundant shell retained by its own trailing run
	let i: ( // c3
		J // c4
	);

	// Union member. The member's own indent is the union printer's, not the shell's
	type K =
		| ( // c5
				L | M
		  )
		| N;

	// Prefix type-operator operand
	type O = keyof ( // c6
		P // c7
	);

	// An author blank below the comment survives the glue — which line the comment sits
	// on and how far it was separated from the type are two different facts
	const q = r as ( // c8

		S // c9
	);

	// Array element, whose `[]` suffix rides outside the pair
	type W = ( // c11
		X extends Y ? Z : A1
	)[];

	// Indexed-access object
	type B1 = ( // c12
		C1 extends D1 ? E1 : F1
	)[G1];

	// Control: written on its own line, it keeps its own line — both authorings are
	// fixed points, and a blank against the `(` is erased in either
	type T = [
		(
			// c10
			U | V
		)?
	];
	type H1 = (
		// c13
		I1 extends J1 ? K1 : L1
	)[];
</script>
