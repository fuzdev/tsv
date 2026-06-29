<script lang="ts">
	// `import`/`export` declarations are `ModuleItem`s — valid only at the module
	// top level (and inside a TS `namespace`/`module` body), never inside a block,
	// function body, or single-statement position (acorn `'import' and 'export'
	// may only appear at the top level`).
	import x from 'y';

	export const a = 1;

	namespace N {
		export const b = 2;
	}
</script>
