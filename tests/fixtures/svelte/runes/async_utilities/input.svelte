<script>
	import { fork, settled, tick } from 'svelte';

	let open = $state(false);
	let pending = null;

	function preload() {
		pending = fork(() => {
			open = true;
		});
	}

	function confirmPreload() {
		pending.commit();
	}

	function cancelPreload() {
		pending.discard();
	}

	async function update() {
		await tick();
		await settled();
	}

	$effect(() => {
		if ($effect.pending()) {
			console.log('loading...');
		}
	});
</script>

<button onclick={preload}>Preload</button>
<button onclick={confirmPreload}>Confirm</button>
<button onclick={cancelPreload}>Cancel</button>
