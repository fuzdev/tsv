<script lang="ts">
	// `<<` splits into instantiation type arguments of a `typeof` query whose
	// first argument is a generic function type — prettier's typescript parser
	// cannot split it here (see prettier_rejects.txt)
	type Y = typeof f<<T>() => void>;
</script>
