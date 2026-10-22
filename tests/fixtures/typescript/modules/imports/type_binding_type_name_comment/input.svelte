<script lang="ts">
	// block comment between the type keyword and a default binding named `type`
	import type /* c1 */ type from './a';

	declare module './m' {
		// block comment between the type keyword and a namespace binding named `type`
		import type /* c2 */ * as type from './b';
	}
</script>
