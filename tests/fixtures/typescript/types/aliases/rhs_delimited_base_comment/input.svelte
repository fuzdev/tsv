<script lang="ts">
	// a value whose comments all sit inside a delimiter it opens first keeps that delimiter on
	// the `=` line, as its comment-free multi-line authoring does
	type A = {
		[K in keyof B]: C;
		// c1
	}[keyof B];

	type D = {
		// c2
		a: B;
	}[keyof B][number];

	type E = {
		[K in keyof B]: C; // c3
	}[];

	type F = [
		B
		// c4
	][number];

	type G = keyof {
		a: B;
		// c5
	};

	// a required paren pair, a `readonly` operator and a suffix after the access answer the same way
	type H = (keyof {
		a: B;
		// c6
	})[K];

	type I = readonly {
		a: B;
		// c7
	}[];

	type J = {
		a: B;
		// c8
	}[K][];

	// a conditional value inside a mapped type, as real code writes it
	type K<T> = {
		[P in keyof T]-?: T[P] extends B // c9
			? P
			: never;
	}[keyof T];

	// a block comment outside the base that stays inline does not break the value: the base's
	// delimiter is still its first break
	type L = keyof /* c10 */ {
		a: B;
		// c11
	};

	type M = {
		a: B;
		// c12
	}[K /* c13 */];

	type N = /* c14 */ {
		a: B;
		// c15
	}[K];
</script>
