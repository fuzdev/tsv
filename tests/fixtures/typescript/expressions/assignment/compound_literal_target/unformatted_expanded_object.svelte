<script lang="ts">
	// a compound operator's left is no pattern: an object or array literal there stays a
	// literal (an invalid target, deferred), methods and accessors included
	({
		a
	} += x);
	({ f() {} } += x);
	({ get f() {} } -= y);
	({ set f(v) {} } &&= z);
	({ *g() {} } += x);
	fn(({ async f() {} } *= y), ([a] += y));
	// a spread or a rest element that a pattern would have to end on
	({
		...a,
		b
	} += x);
	f(([...a, b] += x));
	f(([...a] += x));
	// a default, and an assignment the literal holds
	({ a = 1 } += x);
	({ a: aa, b: bb = 1, c } += x);
	f(([a = 1] += x));
	f(([(a += b)] += x));
	// a default's own value keeps the pair a default takes
	f(([a = (b = c)] += x));
	({ a = (b += c) } += x);
	({ a: b = (c = d) } += x);
	({ a: { b: { c } } = {} } += x);
	({ a = alphaValueNumberOne + betaValueNumberTwo + gammaValueNumberThree + deltaValueNumbe } += x);
	({ a = alphaValueNumberOne + betaValueNumberTwo + gammaValueNumberThree + deltaValueNumber } +=
		x);
	({
		a = aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa +
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb +
			cccccccccccccccccccccccccccccccccc
	} += x);
	// a comment in the pair the parser erased from a default's value
	f(([a = (b = c) /* c */] += x));
	({ a = (b += c) /* c */ } += x);
	({ a: b = (c = d) /* c */, e } += x);
	f(([a = b /* c */, d] += x));
	f(
		([
			a = (b = c), // c
			d
		] += x)
	);
	({
		a = b // c
	} += x);
	// nested
	({
		a: [b]
	} ||= x);
	({
		a: { b = 1 }
	} += x);
	({
		a: [(b -= c)]
	} ||= x);
	f(([[a], { b }] += x));
	({
		alphaName: aa,
		betaName: bb,
		gammaName: cc
	} += someFunctionCall(argumentNumberOne, argumentNumberTwo, argumentNumberThree));
	// comments, a freeze and the destructuring break in the target's own layout
	({ a /* c */: b = 1 } += x);
	({
		k /* c */: {
			b: [c]
		}
	} += x);
	f(([[a, b], [c, d]] += x));
	f(
		([
			// prettier-ignore
			a   =   1
		] += x)
	);
	({
		alphaName,
		betaName,
		gammaName = 1
	} += someFunctionCall(argumentNumberOne, argumentNumberTwo, argumentNumberThree));
	// a freeze over a default whose value shell holds a comment
	f(
		([
			// prettier-ignore
			a   =   (b /* c */),
			d
		] += x)
	);
	f(
		([
			d,
			// prettier-ignore
			a   =   (b
			/* c */
			)
		] += x)
	);
	({
		k:
			// prettier-ignore
			a   =   (b /* c */)
	} += x);
</script>

{f(({ a } += x), ([a] += y))}
