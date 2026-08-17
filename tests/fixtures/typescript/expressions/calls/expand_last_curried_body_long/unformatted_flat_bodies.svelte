<script>
	// exactly 100 - the whole call fits, so the curried chain and its object body stay inline
	fn1(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, (a) => (b) => ({ c: 1, d: 2 }));

	// 101 - the object terminal hugs the callee line and the object expands internally
	fn1(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, (a) => (b) => ({ c: 1, d: 2 }));

	// 101 - an array terminal hugs the same way, with no parens added around the array
	fn1(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, (a) => (b) => [c, d, e]);

	// the heads plus the object opener fit the callee line (exactly 100) - the hug holds
	fn1('first', (argument1) => (argument2) => (argument3) => (argument4) => (argument5aaaaaaaa) => ({ c: 1 }));

	// one char over (101) - every argument breaks out and the chain progressive-indents
	fn1('first', (argument1) => (argument2) => (argument3) => (argument4) => (argument5aaaaaaaaa) => ({ c: 1 }));

	// a destructured head, which forces the chain layout open elsewhere, still hugs here
	fn1('first', ({ a }) => ({ b }) => ({ c: 1, d: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa }));

	// the same heads too wide to hug - the arguments break out and each head takes a line
	fn1('first', ({ aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa }) => ({ bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb }) => ({ c: 1 }));
</script>
