<script lang="ts">
	// a union member's `|` gap, over an array suffix
	type A =
		| B
		// c1
		| C[];

	// the same gap over an indexed access
	type D =
		| E
		// c2
		| F['k'];

	// the same gap where the member is an intersection whose FIRST member is the shell
	type G =
		| H
		// c3
		| (I & J);

	// a tuple element's list gap, over a conditional's check type
	type O = [
		P,
		// c4
		Q extends R ? S : T
	];

	// a type argument's list gap, same check type
	type U = V<
		W,
		// c5
		X extends Y ? Z : A1
	>;

	// the same list gap over an intersection head
	type B1 = C1<
		D1,
		// c6
		E1 & F1
	>;

	// a conditional's `?` branch gap, over a nested conditional's check type
	type G1 = H1 extends I1
		? // c7
			J1 extends K1
			? L1
			: M1
		: N1;

	// its `:` branch twin
	type O1 = P1 extends Q1
		? R1
		: // c8
			S1 extends T1
			? U1
			: V1;

	// the `?` branch over an intersection head
	type W1 = X1 extends Y1
		? // c9
			Z1 & A2
		: B2;

	// its `:` branch twin
	type C2 = D2 extends E2
		? F2
		: // c10
			G2 & H2;

	// a redundant paren LAYER over the shell, and two descent links composed
	type I2 =
		| J2
		// c11
		| K2[]['k'];
</script>
