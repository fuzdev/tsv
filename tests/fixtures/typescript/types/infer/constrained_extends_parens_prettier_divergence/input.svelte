<script lang="ts">
	// Nested-arrow return ending in a constrained infer: the parens are required
	type NestedArrow<M> = M extends (() => () => infer U extends string) ? U : never;

	// Conditional-type infer constraint: the parens are required (without them the
	// enclosing `? :` rebinds)
	type CondConstraint<X> = X extends infer U extends (A extends B ? C : D) ? U : never;

	// Single-arrow constrained-infer return: the parens are required
	type SingleArrow<M> = M extends (() => infer U extends string) ? U : never;
</script>
