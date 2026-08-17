<script lang="ts">
	// `: T` annotation whose type is an ARRAY over a redundant paren shell
	let a: // c1
	A[];

	// the same annotation with an INDEXED-ACCESS suffix
	let b: // c2
	B['k'];

	// type-alias `=` RHS
	type C =
		// c3
		D[];

	// `as` cast
	const e = f as G[]; // c4

	// mapped-type `]:` value
	type H = {
		[
			I in string // c5
		]: J[];
	};

	// type-parameter `=` default
	type K<
		L = // c6
			M[]
	> = L;

	// return-type annotation
	function fn(): // c7
	N[] {
		return [];
	}

	// type-predicate `is`
	function fn2(o: unknown): o is P[] { // c8
		return true;
	}

	// the OWN-LINE placement, at the five gaps whose emitter preserves it — the shell
	// strips either way, and the comment keeps the line it was written on
	type Q =
		// c9
		R[];

	const s = t as // c10
	U[];

	type V = {
		[W in string]: // c11
		X[];
	};

	type Y<
		Z = // c12
		A1[]
	> = Z;

	function fn3(b1: unknown): b1 is // c13
	C1[] {
		return true;
	}

	// the third descent link: a conditional's CHECK type, with and without a suffix
	// between it and the shell
	let d1: // c14
	D1 extends E1 ? F1 : G1;

	let d2: // c15
	D2[] extends E2 ? F2 : G2;

	// a redundant paren LAYER between the gap and the shell — the descent peels it, so an
	// author's extra pair changes nothing, in both placements
	let d3: // c16
	D3[];

	const e3 = f3 as // c17
	G3['k'];
</script>
