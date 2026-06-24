<script lang="ts">
	// A comment between the `from` keyword and the module source is kept in place,
	// even when its own text contains the word `from`.
	import a from /* from */ './a';
	import { b } from /* from */ './b';
	export { c } from /* from */ './c';
	export * from /* from */ './d';
</script>
