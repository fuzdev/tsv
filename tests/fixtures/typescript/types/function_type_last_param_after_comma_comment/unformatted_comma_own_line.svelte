<script lang="ts">
	// a block comment after the last parameter's comma trails the parameter - the comma is
	// never re-emitted, so the comment keeps its line and the signature stays inline
	type A = (a: T
	, /* c */) => void;

	// a constructor type takes the same rule
	type B = new (a: T
	, /* c */) => void;

	// so do the interface member signatures
	interface Single {
		fn(a: T
		, /* c */): void;
		(a: T
		, /* c */): void;
		new (a: T
		, /* c */): void;
	}

	// a tuple, a type-parameter declaration and a type-argument list too
	type C = [T
	, /* c */];
	function fn1<T
	, /* c */>() {}
	const a = fn2<T
	, /* c */>();

	// contrast - a comment the author gave a line of its own keeps it and expands the list
	type D = (a: T,
	/* c */) => void;
</script>
