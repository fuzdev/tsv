<script lang="ts">
	// behind a rename, a reserved word after the `type` modifier names the imported export
	import { type default as d } from './a';
	import { type class as C } from './b';
</script>
