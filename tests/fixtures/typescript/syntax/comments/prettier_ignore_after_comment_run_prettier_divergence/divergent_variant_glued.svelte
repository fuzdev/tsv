<script lang="ts">
	// a comment the author wrote before a comma, with an own-line directive below it: the
	// comment keeps a line of its own, and the directive stays alone on its line directly
	// above what it freezes
	x = [
		a,
		b,
		/* t */ // prettier-ignore
		c  +  d
	];

	fn(
		a,
		b,
		/* t */ // prettier-ignore
		c  +  d
	);

	new A(
		a,
		b,
		/* t */ // prettier-ignore
		c  +  d
	);

	x = {
		a,
		b,
		/* t */ // prettier-ignore
		c: c  +  d
	};

	[
		a,
		b,
		/* t */ // prettier-ignore
		c = 1  +  2
	] = x;

	({
		a,
		b,
		/* t */ // prettier-ignore
		c = 1  +  2
	} = x);

	// a block directive alike
	x = [a, b, /* t */ /* prettier-ignore */ c  +  d];

	// a tuple type, and type arguments and parameters, alike
	type T = [
		a,
		b,
		/* t */ // prettier-ignore
		{  x:  1  }
	];

	f<
		A,
		B,
		/* t */ // prettier-ignore
		{  x:  1  }
	>();

	function g<
		A,
		B,
		/* t */ // prettier-ignore
		C  extends  D
	>() {}
</script>
