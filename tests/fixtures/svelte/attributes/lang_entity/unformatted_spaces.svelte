<script  lang="&#116;s" >
	// The one reader that must NOT decode: the wire schema follows Svelte's own raw-bytes
	// `lang` regex, so this script is not TypeScript. The Svelte import shape in
	// expected.json — `attributes` present, no `importKind` — is the assertion.
	import a  from 'b';
</script>

<!-- The `lang` / `type` gate reads the attribute's DECODED value, so an entity-spelled one
	routes exactly like its plain spelling. -->

<!-- `lang="&#104;tml"` decodes to `html`, so the body formats as markup -->
<template  lang="&#104;tml" >
	<div   >text</div>
</template>

<!-- `type="text&#47;html"` decodes to `text/html`, whose `text/` prefix strips to `html` -->
<template  type="text&#47;html" >
	<div   >text</div>
</template>

<!-- `lang="&#99;ss"` decodes to `css`, so the stylesheet formats as CSS -->
<style  lang="&#99;ss" >
	div {
		color:   red;
	}
</style>
