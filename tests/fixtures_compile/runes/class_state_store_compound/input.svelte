<script>
	import { writable } from 'svelte/store';
	const count = writable(0);
	class C {
		x = $state($count + 1);
	}
	const c = new C();
</script>

<p>{c.x}</p>
