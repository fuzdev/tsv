<script lang="ts">
	// Block comments before `as` and before `namespace` — each stays where the author
	// wrote it; prettier relocates both past the keyword
	export /* c1 */ as /* c2 */ namespace Foo;
</script>
