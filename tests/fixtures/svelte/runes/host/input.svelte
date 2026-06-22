<svelte:options customElement="my-element" />

<script lang="ts">
	function greet(greeting: string) {
		$host().dispatchEvent(new CustomEvent('greeting', { detail: greeting }));
	}
</script>

<button onclick={() => greet('hello')}>say hello</button>
