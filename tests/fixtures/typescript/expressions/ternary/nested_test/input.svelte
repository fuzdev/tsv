<script lang="ts">
	// a ternary in TEST position keeps its parens: `?:` is right-associative, so bare the
	// test would re-bind into the enclosing ternary as its alternate
	const a = (b ? c : d) ? e : f;

	// nested one level further down the test spine
	const g = ((h ? i : j) ? k : l) ? m : n;

	// the test of a ternary that is itself a branch
	const o = p ? ((q ? r : s) ? t : u) : v;

	// a member base and a call callee take the same pair
	const w = (x ? y : z).k;
	const aa = (bb ? cc : dd)();

	// 100 chars - the parenthesized test stays inline, the outer ternary's arms already broken
	const oo = (ppppppppppppppppppppppppppp ? qqqqqqqqqqqqqqqqqqqqqqqqqq : rrrrrrrrrrrrrrrrrrrrrrrrrr)
		? ss
		: tt;

	// 101 chars - the pair EXPANDS and the test drops to its own indented line
	const uu = (
		vvvvvvvvvvvvvvvvvvvvvvvvvvvv ? wwwwwwwwwwwwwwwwwwwwwwwwww : xxxxxxxxxxxxxxxxxxxxxxxxxx
	)
		? yy
		: zz;
</script>
