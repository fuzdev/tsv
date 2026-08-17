<script lang="ts">
	// A `//` the author glued to a union-member object's `{` keeps that line, as it
	// does at the plain type literal below
	type T =
		| {// c1
				a: A;
		  }
		| B;

	// The parenthesized-intersection object shares the alignment rendering
	type U =
		| (C & {   // c2
				b: B;
		  })
		| D;

	// Control - the plain type literal, whose own path already answered this way
	type V = {  // c3
		c: C;
	};

	// Control - written on its own line the comment keeps its own line, and
	// prettier agrees
	type W =
		| {
				// c4
				d: D;
		  }
		| E;
</script>
