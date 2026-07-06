<script lang="ts">
	// A contextual type keyword as the HEAD of a qualified type name is a
	// TSQualifiedName (TSTypeReference), not the primitive keyword
	let a: string.X;
	type T = number.A.B;
	let c: object.K;
	let e: undefined.M;

	// A keyword head may carry type arguments
	type G = any.Box<number>;

	// A bare keyword stays the primitive type (not qualified)
	let d: string;
	type U = number;
</script>
