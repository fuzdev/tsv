<script lang="ts">
	// `<<` is the left-shift operator in expression position
	const a1 = a << b;
	const a2 = (a << 2) >> c;

	// a shift then a comparison — `b > c` is a relational chain, not a
	// type-argument close (no `=>` after the would-be close)
	const a3 = a << b > (c);

	// `<<` splits into `<` `<` when type arguments open with a generic function type
	const b1 = fn<<T>(v: T) => void>();
	const b2 = new Fn<<T>(v: T) => void>();
	const b3 = fn<<T>(v: T) => void, U>(x);

	// the same split in an optional call, a bare instantiation expression, and
	// with an untyped parameter
	const b4 = fn?.<<T>(v: T) => void>();
	const b5 = fn<<T>(v: T) => void>;
	const b6 = fn<<T>(v) => void>();

	// the same split at a type position
	type X = A<<T>() => R>;
</script>
