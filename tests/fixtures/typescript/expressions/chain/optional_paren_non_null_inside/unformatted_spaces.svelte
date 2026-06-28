<script lang="ts">
	// The non-null `!` authored INSIDE the sealing parens (`(a?.b!).c`) is preserved
	// as written — prettier 3.9 (#18661) stopped lifting it to the `!`-outside form
	// `(a?.b)!.c`. The two are semantically identical (the `!` is type-only) but have
	// different ASTs, so tsv keeps the author's placement.
	( a?.b! ) . c ;
	( a?.b! ) (  ) ;
	( a?.b! ) [ c ] ;
	// `!` inside, after an optional call
	( a?.()! ) . c ;
	// `!` inside, then a further member chain
	( a?.b! ) . c . d ;
</script>
