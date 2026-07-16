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
</script>
