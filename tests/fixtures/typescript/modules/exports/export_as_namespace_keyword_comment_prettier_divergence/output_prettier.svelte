<script lang="ts">
	// Block comments before `as` and before `namespace` — each stays where the author
	// wrote it; prettier relocates both past the keyword
	export as namespace /* c1 */ /* c2 */ Foo;
</script>
