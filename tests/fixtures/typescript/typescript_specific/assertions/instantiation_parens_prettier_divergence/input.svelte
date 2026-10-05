<script lang="ts">
	// Assignment in instantiation: parens required (both formatters agree)
	let a = (x = y)<T>;
	let b = (x = y)<T1, T2>;

	// Ternary in instantiation: parens preserve semantics
	// `(x ? y : z)<T>` (instantiate ternary result) vs `x ? y : z<T>` (instantiate alternate only)
	let c = (x ? y : z)<T>;

	// Binary in instantiation: parens preserve semantics
	// `(a + b)<T>` (instantiate sum) vs `a + b<T>` (add to instantiated b)
	let d = (a + b)<T>;

	// Unary in instantiation: parens preserve semantics
	// `(typeof e)<T>` (instantiate the operator's result) vs `typeof e<T>` (the operator over an instantiated `e`)
	let f = (typeof e)<T>;
	let g = (-e)<T>;
	let h = (!e)<T>;
	let i = (void e)<T>;
</script>
