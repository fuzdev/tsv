<script lang="ts">
	import type { Foo } from './foo.ts';
	import { bar } from './bar.ts';

	let x: Foo = bar;
</script>

<p>{x}</p>
