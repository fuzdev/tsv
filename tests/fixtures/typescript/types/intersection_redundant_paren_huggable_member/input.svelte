<script lang="ts">
	// a redundant paren around a huggable member is dropped and changes no layout: the
	// `&` still hugs the member's `}` at every position an intersection reaches

	// first member
	type First = {
		a: 1;
	} & B;

	// last member
	type Last = B & {
		a: 1;
	};

	// a mapped type is huggable too
	type Mapped = {
		[K in T]: U;
	} & B;

	// nested: a generic argument
	type Nested = G<
		{
			a: 1;
		} & B
	>;

	// nested: an interface member
	interface Member {
		a: {
			b: 1;
		} & B;
	}

	// three members: the boundary past index 1 carries the continuation indent
	type ThreeFirst = {
		a: 1;
	} & B &
		C;

	// three members with the object in the middle
	type ThreeMid = B & {
		a: 1;
	} & C;
</script>
