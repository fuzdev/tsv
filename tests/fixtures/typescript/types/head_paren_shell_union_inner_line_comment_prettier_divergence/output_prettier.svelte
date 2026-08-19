<script lang="ts">
	// the head shell wraps a UNION, whose pair the member position REQUIRES - so the layer
	// the author added collapses into that one pair and the run stays inside it, wherever
	// the intersection sits. an optional tuple element:
	type A = [
		(// c1
		(B | C) &
			D)?
	];

	// a union member
	type E =
		| F
		| (// c2
		  (G | H) &
				I);

	// an array element
	type J = (// c3
	(K | L) &
		M)[];

	// an indexed access's object
	type N = (// c4
	(O | P) &
		Q)['k'];

	// a conditional's check type
	type R = // c5
	(S | T) &
		U extends V
		? W
		: X;

	// control: the plain alias value, which lands on the same form
	type Y = // c6
	(Z | A1) &
		B1;
</script>
