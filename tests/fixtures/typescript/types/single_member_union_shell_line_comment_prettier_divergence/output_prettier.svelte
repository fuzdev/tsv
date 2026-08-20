<script lang="ts">
	// a one-member union prints as its member, so a `//` in that member's shell changes
	// only where it renders - never which parens survive, and never a `|` of its own
	type A =
		// c1
		B | C;

	// the same run one redundant layer deeper
	type D =
		// c2
		E | F;

	// a type argument
	type G = q<
		// c3
		H | I
	>;

	// an object type's value
	type J = {
		p:
			// c4
			K | L;
	};

	// a tuple element
	type M = [
		// c5
		N | O
	];

	// an indexed access's object
	type P = (
		// c6
		Q | R
	)['k'];

	// a conditional's check type
	type S =
		// c7
		T | U extends V ? W : X;

	// the member's own pair is REQUIRED, so it survives and keeps the comment inside
	type Y = // c8
	(Z | A1) &
		B1;

	// control: the same authoring without the leading `|`, which lands on the same form
	type C1 =
		// c9
		D1 | E1;
</script>
