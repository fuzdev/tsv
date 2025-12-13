<script lang="ts">
	// Basic infer in conditional type
	type Unpromise<T>=T extends Promise<infer U>?U:never;

	// Infer return type
	type GetReturnType<T>=T extends (...args:any[])=>infer R?R:never;

	// Infer array element
	type Flatten<T>=T extends Array<infer Item>?Item:T;

	// Infer first argument
	type FirstArg<T>=T extends (first:infer F,...rest:any[])=>any?F:never;

	// Multiple infers
	type Both<T>=T extends {a:infer A;b:infer B}?[A,B]:never;

	// Nested infer
	type DeepUnwrap<T>=T extends Promise<infer U>?(U extends Promise<infer V>?V:U):T;
</script>
