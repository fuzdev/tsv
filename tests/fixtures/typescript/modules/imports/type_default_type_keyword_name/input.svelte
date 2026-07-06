<script lang="ts">
	// A type-only default import binding may be a contextual type keyword
	import type any from './a';
	import type string from './b';
	import type number from './c';

	// Namespace type-only import with a contextual keyword binding
	import type * as object from './e';

	// `import type from 'x'` is NOT a type-only import: it is a value import of a
	// default binding named `type` (the `from` disambiguation is preserved)
	import type from './d';
</script>
