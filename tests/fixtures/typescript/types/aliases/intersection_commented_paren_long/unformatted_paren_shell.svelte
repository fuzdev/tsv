<script lang="ts">
	// leading block comment in the shell - 100 chars: the intersection stays on the `=` line
	type A = (/* c */ XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
		a: A;
	});

	// leading block comment in the shell - 101 chars: the intersection breaks after `=`
	type B = (/* c */ XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
		a: A;
	});

	// trailing block comment in the shell - 100 chars
	type C = (XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
		a: A;
	} /* c */);

	// trailing block comment in the shell - 101 chars
	type D = (XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
		a: A;
	} /* c */);

	// type parameters - 100 chars
	type E<T extends U> = (/* c */ XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
		a: A;
	});

	// type parameters - 101 chars
	type F<T extends U> = (/* c */ XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
		a: A;
	});

	// a single-member union - 100 chars
	type G<T extends U> = | (/* c */ XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
		a: A;
	});

	// a single-member union - 101 chars
	type H<T extends U> = | (/* c */ XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX<T> & {
		a: A;
	});
</script>
