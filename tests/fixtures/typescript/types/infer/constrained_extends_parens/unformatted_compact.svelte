<script lang="ts">
	// Function-type extends operand: a constrained-infer return keeps its parens
	// (without them the infer's `extends C` and the conditional's `?` are ambiguous)
	type FnInfer<M> = M extends (()=>infer U extends string)?U:never;

	// Type-predicate return whose asserted type is a constrained infer also keeps parens
	type PredInfer<M> = M extends ((x:string)=>x is infer U extends string)?U:never;

	// Constructor-type extends operand, same rule
	type CtorInfer<M> = M extends (new ()=>infer U extends number)?U:never;

	// Contrast: a bare `infer U` return (no constraint) needs no parens
	type BareInfer<M> = M extends (()=>infer U)?U:never;

	// Contrast: a simple return type needs no parens
	type SimpleRet<M> = M extends (()=>string)?1:2;
</script>
