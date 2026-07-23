<script lang="ts">
	// A constrained `infer U extends T` as a union member keeps its parens: without
	// them the `extends` constraint greedily absorbs the following `| {…}`, widening
	// the constraint from `number` to `number | {…}`.
	type UnionFirst<T> = T extends (infer U extends number) | { a: 1 } ? U : never;

	// Same rule in the last position — prettier keeps the parens here too.
	type UnionLast<T> = T extends { a: 1 } | (infer U extends number) ? U : never;

	// Intersection member — same greedy `&` absorption, same required parens.
	type IntersectionFirst<T> = T extends (infer U extends number) & { a: 1 } ? U : never;

	type IntersectionLast<T> = T extends { a: 1 } & (infer U extends number) ? U : never;

	// Contrast: a bare `infer U` (no constraint) has nothing to absorb, so no parens.
	type BareInfer<T> = T extends infer U | { a: 1 } ? U : never;
</script>
