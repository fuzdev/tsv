<script lang="ts">
	// Plain call, expression body - comment between the signature and `=>`
	fn((a) /* c1 */ => call(a));

	// With a return type annotation
	fn((a: string): void /* c2 */ => call(a));

	// With type parameters
	fn(<T extends string>(a: T) /* c3 */ => call(a));

	// A `new` expression's arguments take the same path
	new Comp((a) /* c4 */ => call(a));

	// A member chain's arguments take the same path
	arr.map((a) /* c5 */ => call(a));

	// Block body - the comment is not a param comment, so the callback still hugs
	fn((a) /* c6 */ => {
		return a;
	});

	// Object body - same
	fn((a) /* c7 */ => ({ a }));

	// After a return type, block body
	fn((a: number): number /* c8 */ => {
		return a;
	});

	// Inside the return-type gap - also not a param comment
	fn((a: number): /* c9 */ number => {
		return a;
	});
</script>
