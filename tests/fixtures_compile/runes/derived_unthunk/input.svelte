<script>
	import { get_library } from './library.ts';

	const library = $derived(get_library());
	const scoped = $derived(get_library().name);
</script>

<p>{library}{scoped}</p>
