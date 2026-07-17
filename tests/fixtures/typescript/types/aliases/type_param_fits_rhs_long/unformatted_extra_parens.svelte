<script lang="ts">
	// A constrained type param whose header fits stays inline even when the whole
	// alias overflows — it is the RHS that breaks, not the `<…>` list. tsv used to
	// break the `<…>` here; prettier keeps it inline.

	// Fully inline contrast: short header + short RHS, one line.
	type Short<T extends Aaaaa | Bbbbb> = T extends Cccc ? Dddd : Eeee;

	// Conditional check fits the `=` line at 100 columns: the ternary wraps in place.
	type CondFits<Param extends Aaaa | Bbbb> = Param extends CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC
		? Cn
		: Al;
	// One column longer: the check no longer fits, so the RHS breaks after `=`.
	type CondBreak<Param extends Aaaa | Bbbb> =
		Param extends CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC ? Cn : Al;

	// Intersection RHS whose first member overflows the `=` line: RHS breaks after `=`.
	type Inter<Param extends Aaaa | Bbbb> =
		MMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMM & Nnnnnnnnnn & Ooooooooo;

	// Function-type RHS (real world: kit's HandleValidationError): RHS breaks after `=`.
	type HandleValidationError<Issue extends SchemaValidationIssue1 = SchemaValidationIssue1> =
		(input: { issues: Issue[] }) => MaybePromiseAppError;

	// Object-literal RHS hugs `= {` and expands internally (its first break point is
	// reachable within the width); the constrained header still stays inline.
	type Obj<Param extends Aaaa | Bbbb> = {
		aaaaaaaaaaaaaaaaaaaaaaaaa: Nnnnn;
		bbbbbbbbbbbbb: Ooooooo;
	};

	// Type-reference RHS hugs `= Foo<` and breaks its type args; header stays inline.
	type Ref<Param extends Aaaa | Bbbb> = Foooooooo<
		Aaaaaaaaaaaaaaaaaaaaaaaaa,
		Bbbbbbbbbbbbbbbbbbbbbbbbb
	>;

	// Postfix-operator RHS: a `[]` (array) or `[K]` (indexed-access) wrapper
	// hides the inner conditional/function from the internal-breaking check, but the
	// RHS still hugs `= (` and breaks INSIDE the parens (prettier's `fluid`) — the
	// `<…>` header staying inline, exactly like the bare conditional/function cases
	// above. tsv used to break after `=`.

	// Array of a conditional: hug `= (`, break the ternary, `)[]` rides the close.
	type ArrCond<Param extends Aaaa | Bbbb> = (Param extends Ccccccccccccccccccc
		? Tnnnnnnnnnnnnn
		: Fnnnnnnnnnnnnn)[];
	// Array of a function type: hug `= ((`, break the parameter list.
	type ArrFn<Param extends Aaaa | Bbbb> = ((
		firstArgument: AaaaaaaaaType,
		secondArgument: BbbbbbbbbType
	) => Rrrrrrrrrrr)[];
	// Indexed access of a conditional: hug `= (`, break the ternary, `)['key']` rides it.
	type IdxCond<Param extends Aaaa | Bbbb> = (Param extends Cccccccccccccccc
		? Tnnnnnnnnnnnn
		: Fnnnnnnnnnnnn)['key'];

	// A union that needs a delimiter — parens as an array element / indexed-access
	// object, brackets as an indexed-access index — EXPANDS that delimiter when it
	// breaks: the union rides an indented line rather than gluing the leading `|` to
	// the `(` / `[` (prettier's `printUnionType`). Inside the expanded delimiter the
	// union stays inline if it fits and breaks to one `| member` per line otherwise;
	// either way the `<…>` header stays inline.

	// Array element union that fits at the new indent: parens expand, union inline.
	type UnionArr<Param extends Aaaa | Bbbb> = (
		'firstLongMemberName' | 'secondLongMemberName' | 'thirdLongMemberNameX'
	)[];
	// Array element union too wide even indented: parens expand and the union breaks
	// to one `| member` per line.
	type UnionArrWide<Param extends Aaaa | Bbbb> = (
		| 'firstVeryLongMemberNameHere'
		| 'secondVeryLongMemberNameHere'
		| 'thirdVeryLongMemberNameHere'
		| 'fourthVeryLongMemberNameHere'
	)[];
	// Indexed-access object union: parens expand, then `['key']`.
	type UnionIdxObj<Param extends Aaaa | Bbbb> = (
		'firstLongMemberName' | 'secondLongMemberName' | 'thirdLongMemberNameX'
	)['key'];
	// Indexed-access index union: the bracket expands but the `]` hugs the last member.
	type UnionIdxKey<Param extends Aaaa | Bbbb> = SomeLongContainerTypeName[('firstLongKeyName' | 'secondLongKeyName' | 'thirdLongKeyNameXX')];

	// A prefix type operator (`keyof`/`readonly`) RHS is `fluid` too: the operator and
	// its operand hug the `=` line and break INSIDE the operand — the `<…>` header
	// staying inline — rather than breaking after `=`. tsv used to break after `=`.

	// keyof over a parenthesized conditional: hug `= keyof (`, break the ternary.
	type KeyofCond<Param extends Aaaa | Bbbb> = keyof (Param extends Ccccccccccccc
		? Tnnnnnnnnnnnn
		: Fnnnnnnnnnnnn);
	// keyof over a union: the required parens EXPAND, the union rides an indented line.
	type KeyofUnion<Param extends Aaaa | Bbbb> = keyof (
		'firstLongMemberName' | 'secondLongMemberName' | 'thirdLongMemberNameX'
	);
	// readonly array of a union: the parens expand, `[]` rides the close.
	type ReadonlyUnionArr<Param extends Aaaa | Bbbb> = readonly (
		'firstLongMemberName' | 'secondLongMemberName' | 'thirdLongMemberX'
	)[];
</script>
