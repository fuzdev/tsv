<script lang="ts">
	// A `this is T` predicate's type runs on to the right, as a function type's return
	// does, so a predicate keeps its pair wherever a token follows it
	type A1 = ((this is B))[];
	type A2 = ((this is B))[K];
	type A3 = C[((this is B))[K]];
	type A4 = ((this is B))[][];
	type A5 = readonly ((this is B))[];
	type A6 = ((this is B | C))[];
	type A7 = ((this is B[]))[K];

	// A union or intersection member, a conditional's check type, an optional element
	type B1 = ((this is B)) | C;
	type B2 = C | ((this is B)) | D;
	type B3 = ((this is B)) & C;
	type B4 = ((this is B)) extends C ? D : E;
	type B5 = [((this is B))?];

	// A last member and an operator's operand take the pair a function type takes there,
	// written or not: bare, they read alike only where nothing follows the whole type
	type C1 = C | ((this is B));
	type C2 = keyof ((this is B));
	type C3 = keyof ((this is B)) | C;
	type C4 = C | ((this is B)) extends D ? E : F;

	// A comment inside the pair stays inside it
	type D1 = ((/* c */ this is B))[];
	type D2 = ((this is B /* c */))[];
	type D3 = ((
		this is B // c
	))[];

	// An annotation, a type argument, a return type
	let e1: ((this is B))[];
	fn<((this is B))[]>(x);
	function fn2(this: unknown): ((this is B))[] {}

	// Where the predicate ends its type, the pair strips
	type F1 = () => this is B;
	type F2 = A<this is B>;
	type F3 = C extends this is B ? D : E;
</script>
