<script lang="ts">
	// a frozen member whose separator sits below a trailing line comment keeps it there
	type A = {
		// prettier-ignore
		a:  1 // c
		;
		b: 1;
	};
	type B = {
		// prettier-ignore
		[k:  string]: 1 // c
		;
		// prettier-ignore
		m(  ): void // c
		;
		// prettier-ignore
		(  ): void // c
		;
		// prettier-ignore
		new (  ): B // c
		;
		// prettier-ignore
		get a(  ): 1 // c
		;
		// prettier-ignore
		set a(  v  ) // c
		;
		b: 1;
	};
	// a block comment ahead of the line comment, a second line comment, an own-line one
	type C = {
		// prettier-ignore
		a:  1 /* c */ // d
		;
		// prettier-ignore
		b:  1 // c
		// d
		;
		// prettier-ignore
		c:  1
		// c
		;
		d: 1;
	};
	// a comment whose text ends in a NEL, which is comment text rather than a line end
	type D = {
		// prettier-ignore
		a:  1 // c
		;
		b: 1;
	};
	// a comma separator, a blank line before the separator, and two frozen members in a row,
	// the second the last with no separator
	type E = {
		// prettier-ignore
		a:  1 // c
		,
		// prettier-ignore
		b:  1 // c

		;
		// prettier-ignore
		c:  1 // c
		;
		// prettier-ignore
		d:  1 // d
	};
	// trailing whitespace inside the comment
	type F = {
		// prettier-ignore
		a:  1 // c   
		;
		b: 1;
	};
	let v: {
		// prettier-ignore
		a:  1 // c
		;
		b: 1;
	};
	function f(a: {
		// prettier-ignore
		a:  1 // c
		;
		b: 1;
	}) {}
</script>
