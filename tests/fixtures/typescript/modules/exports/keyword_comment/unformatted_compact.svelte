<script lang="ts">
	// block comment between export and braces (empty)
	export/* c */{};

	// dangling comment inside empty braces (relocated before braces)
	export{/* c */};

	// block comment between from and source (re-export)
	export{}from/* c */'a';

	// dangling comment inside empty braces with re-export (relocated after from)
	export{/* c */}from'c';

	// block comment between from and source (export all)
	export*from/* c */'d';

	// block comment between from and source (with specifiers)
	export{a}from/* c */'f';
</script>
