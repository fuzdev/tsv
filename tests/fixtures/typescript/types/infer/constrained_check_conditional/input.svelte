<script lang="ts">
	// At an allow-conditional position, `infer U extends number ?` starts a
	// conditional: the `extends number` re-binds as the conditional's extends
	// clause and the check is the bare `infer U` (the TS constraint rollback rule)
	type Paren<T> = T extends (infer U extends number ? U : T) ? U : T;

	// Same re-binding at the top level of the alias value
	type TopLevel<T> = infer U extends number ? U : T;

	// Inside a tuple member
	type Tuple<T> = T extends [infer U extends number ? U : T] ? U : T;

	// Inside an object-type member
	type Obj<T> = T extends { a: infer U extends B ? U : T } ? U : T;

	// Inside a type argument
	type TypeArg<T> = T extends Array<infer U extends number ? U : T> ? U : T;

	// The check can be a union ending in a constrained infer (`A | infer U`)
	type UnionCheck<T> = T extends (A | infer U extends B ? U : T) ? U : T;

	// A function-type return is an allow position: the conditional is the return type
	type FnReturn<T> = () => infer U extends number ? U : T;
</script>
