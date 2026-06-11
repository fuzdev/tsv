<script lang="ts">
	// Nested-arrow return ending in a constrained infer: the parens are required
	// (prettier strips them and emits unparseable output)
	type NestedArrow<M> = M extends (() => () => infer U extends string) ? U : never;

	// Conditional-type infer constraint: the parens are required (without them the
	// enclosing `? :` rebinds); prettier strips them — also unparseable
	type CondConstraint<X> = X extends infer U extends (A extends B ? C : D) ? U : never;

	// Sibling that does NOT diverge: a single-arrow constrained-infer return keeps
	// its parens in both formatters
	type SingleArrow<M> = M extends (() => infer U extends string) ? U : never;
</script>
