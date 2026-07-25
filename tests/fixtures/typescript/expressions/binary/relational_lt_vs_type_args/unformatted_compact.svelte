<script lang="ts">
	// `<` before a literal is a comparison operator, not type arguments
	const a1=x<'b';
	const a2=x<'b'&&y<'c';
	const a3=x<`b`;
	const a4=x<true;
	const a5=x<null;
	const a6=x<{a:1};
	const a7=x<[1];

	// a closing `>` followed by an identifier is a chained comparison, not type args;
	// this holds across string, numeric, and bare-identifier operands
	const a8=x<'b'>c;
	const a9=x<1>c;
	const a10=a<b>c;
	const a11=a<b<c;

	// a parenthesized ternary operand is a comparison, not a `(b?: T) => ...` function type
	const a12=x<(b?c:d);

	// a `>` inside an array literal belongs to the array, not the relational chain
	const a13=x<[a>b];

	// any expression-starting token after the closing `>` makes it a comparison
	// chain too — literal, unary, array, and object operands behave like the
	// identifier and typeof operands
	const a14=x<y>1;
	const a15=x<y>'b';
	const a16=x<y>!c;
	const a17=x<y>~c;
	const a18=x<y>-1;
	const a19=x<y>+1;
	const a20=x<y>[0];
	const a21=x<y>{a:1};
	const a22=x<y>typeof c;

	// an indexed member access after `<` is a comparison operand, not an indexed access
	// type; the `,` that follows it belongs to the enclosing argument, element, or
	// property list, whatever the index is
	fn(a<B[c],d,);
	fn(a<B['k'],d);
	fn(a<B[typeof c],d);
	const a23=[a<B[c],d];
	const a24={a:b<C[d],e:f};
	const a25=(a<B[c],d);
	const a26=a<B.C[d];
	const a27=a<B[c][e];

	// a `>` or a shift operator after the indexed operand continues the comparison
	// chain — the `>` run belongs to the operator, not to a type-argument close
	const a28=a<B[c]>d;
	const a29=a<B[c]>>d;
	const a30=a<B[c]>>>d;

	// an identifier that merely starts with `extends` is a fresh statement on the next
	// line (ASI), not a type constraint
	const a31=a<b;
	extendsFoo();

	// genuine instantiation with a literal/keyword/object/tuple/numeric type arg, or a
	// function type with an optional param, still parses as type arguments
	const b1=fn<'b'>();
	const b2=fn<true>();
	const b3=fn<string>();
	const b4=fn<{a:number}>();
	const b5=fn<[A,B]>();
	const b6=fn<1>();
	const b7=fn<(b?:T)=>U>();

	// a generic nested inside a tuple or object-type arg still parses as type arguments
	const b8=fn<[T<A>]>();
	const b9=fn<[A,T<B>]>();
	const b10=fn<{a:T<B>}>();
	const b11=fn<[T<A,B>]>();
	const b12=fn<[T<A>[]]>();
	const b13=fn<[T<A>],B>();
	const b14=fn<[[T<A>]]>();

	// a template literal after the closing `>` is a tagged template on the
	// instantiation, not a comparison
	const b15=fn<A>`b`;

	// an indexed access, array, or conditional type argument still parses as type
	// arguments — the closing `>` is followed by a call
	const b16=fn<A[]>();
	const b17=fn<A['b']>();
	const b18=fn<A[B],C>();
	const b19=fn<A[keyof B]>();
	const b20=fn<A[typeof b]>();
	const b21=fn<A[B][C]>();
	const b22=fn<T extends U?A:B>();
</script>
