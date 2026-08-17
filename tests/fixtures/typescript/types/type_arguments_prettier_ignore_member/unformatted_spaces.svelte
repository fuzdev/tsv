<script lang="ts">
	// an own-line directive in a type-argument list freezes only the following
	// argument — type position
	type A = Foo<
		// prettier-ignore
		{x:   1},
		b
	>;

	// between arguments
	type B = Foo<
		a ,
		// prettier-ignore
		{x:   1},
		c
	>;

	// call-site type arguments
	const r = fn<
		// prettier-ignore
		{x:   1},
		b
	>(arg);

	// new-expression type arguments
	const s = new Cls<
		// prettier-ignore
		{x:   1},
		b
	>(arg);

	// a plain glued block comment on a sole argument is preserved
	const t = fn3< /* c */ { x : 1 }>(arg);

	// own-line directive on a sole argument: the list expands (a frozen argument
	// never hugs the angle brackets — the directive stays own-line)
	type C = Foo<
		// prettier-ignore
		{x:   1}
	>;
</script>
