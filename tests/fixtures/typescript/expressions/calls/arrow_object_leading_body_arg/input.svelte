<script lang="ts">
	// an arrow argument whose body begins with an object literal keeps the pair around it
	// a ternary
	fn('t', (a) => ({}).x ? b : c);
	fn((a) => ({}).x ? b : c);
	fn((a) => ({}).x ? b : c, 't');
	fn()('t', (a) => ({}).x ? b : c);
	x.y().z('t', (a) => ({}).x ? b : c);
	new Fn('t', (a) => ({}).x ? b : c);
	// a ternary on the object
	fn('t', (a) => ({}) ? b : c);
	fn((a) => ({}) ? b : c);
	fn((a) => ({}) ? b : c, 't');
	fn()('t', (a) => ({}) ? b : c);
	x.y().z('t', (a) => ({}) ? b : c);
	new Fn('t', (a) => ({}) ? b : c);
	// a member
	fn('t', (a) => ({}).x);
	fn((a) => ({}).x);
	fn((a) => ({}).x, 't');
	fn()('t', (a) => ({}).x);
	x.y().z('t', (a) => ({}).x);
	new Fn('t', (a) => ({}).x);
	// a call
	fn('t', (a) => ({}).f(a));
	fn((a) => ({}).f(a));
	fn((a) => ({}).f(a), 't');
	fn()('t', (a) => ({}).f(a));
	x.y().z('t', (a) => ({}).f(a));
	new Fn('t', (a) => ({}).f(a));
	// a binary
	fn('t', (a) => ({}).x + b);
	fn((a) => ({}).x + b);
	fn((a) => ({}).x + b, 't');
	fn()('t', (a) => ({}).x + b);
	x.y().z('t', (a) => ({}).x + b);
	new Fn('t', (a) => ({}).x + b);
	// a cast
	fn('t', (a) => ({}) as T);
	fn((a) => ({}) as T);
	fn((a) => ({}) as T, 't');
	fn()('t', (a) => ({}) as T);
	x.y().z('t', (a) => ({}) as T);
	new Fn('t', (a) => ({}) as T);
	// a non-null member
	fn('t', (a) => ({})!.x);
	fn((a) => ({})!.x);
	fn((a) => ({})!.x, 't');
	fn()('t', (a) => ({})!.x);
	x.y().z('t', (a) => ({})!.x);
	new Fn('t', (a) => ({})!.x);
	// a tagged template
	fn('t', (a) => ({}).t`x`);
	fn((a) => ({}).t`x`);
	fn((a) => ({}).t`x`, 't');
	fn()('t', (a) => ({}).t`x`);
	x.y().z('t', (a) => ({}).t`x`);
	new Fn('t', (a) => ({}).t`x`);

	// the body broken after `=>`
	fn('tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		({}).x ? b : c
	);
	ftttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt((a) =>
		({}).x ? b : c
	);
	x.y().z('ttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		({}).x ? b : c
	);
	new Fn('tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		({}).x ? b : c
	);
	fn()('tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		({}).x ? b : c
	);
	fn('tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		({}).f(a)
	);
	ftttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt((a) =>
		({}).f(a)
	);
	x.y().z('ttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		({}).f(a)
	);
	new Fn('tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		({}).f(a)
	);
	fn()('tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		({}).f(a)
	);

	// a lone call body on a member chain, and the chain broken
	x.y().ztttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt((a) =>
		({}).f(a)
	);
	x.y('tttttttttttttttttttttttttttttt', (a) => ({}).f(a))
		.z('tttttttttttttttttttttttttttttt', (b) => ({}).g(b))
		.w((c) => c);
	aaa.bbb.ccc('tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		({}).f(a)
	);
	// a ternary on the object, the `=>` line at 100 chars - the body breaks (no space is measured)
	fn('ttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) =>
		({}).x ? b : c
	);
	// the `=>` line at 101 chars - the arguments break
	fn(
		'tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt',
		(a) => ({}).x ? b : c
	);

	// a lone argument, the `=>` line at 101 chars - the argument breaks out
	fttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt(
		(a) => ({}).x ? b : c
	);
	fttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt(
		(a) => ({}).f(a)
	);

	// an own-line comment before the body
	fn('t', (a) =>
		// c
		({}).x ? b : c
	);
	fn(
		'tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt',
		(a) =>
			// c
			({}).x ? b : c
	);
	fn('t', (a) =>
		// c
		({}).f(a)
	);
	fn(
		'tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt',
		(a) =>
			// c
			({}).f(a)
	);
	fn(
		't',
		(a) =>
			// c
			({}).x
	);
	fn(
		'tttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttttt',
		(a) =>
			// c
			({}).x
	);
</script>

<div onclick={(a) => ({}).x ? b : c}></div>
<Comp on={(a) => ({}).f(a)} />
