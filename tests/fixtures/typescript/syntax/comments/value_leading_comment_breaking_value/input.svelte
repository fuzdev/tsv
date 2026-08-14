<script lang="ts">
	// type alias value: the comment's soft separator breaks with the value — run own-line under `=`
	type A =
		/* c */
		{
			a: 1;
			b: 2;
			c: 3;
		};

	// variable initializer: same rule
	const y =
		/* c */
		{
			a: 1,
			b: 2
		};

	// variable type annotation: the run stays on the head line, the value opens on the next
	let x: /* c */
	{
		a: 1;
		b: 2;
	};

	// function return type: same rule as the annotation
	function fn(): /* c */
	{
		a: 1;
	} {
		return { a: 1 };
	}

	// a comment the author glued to the value hugs it through the break
	type B = /* g */ {
		a: 1;
		b: 2;
		c: 3;
	};

	// glued on an annotation: same hug
	let z: /* g */ {
		a: 1;
		b: 2;
	};
</script>
