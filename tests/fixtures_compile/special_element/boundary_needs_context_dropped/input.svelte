<script>
	class C {}
</script>

<svelte:boundary>{@html new C()}{#snippet pending()}<i>w</i>{/snippet}</svelte:boundary>
