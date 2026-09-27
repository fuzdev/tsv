<!-- format-ignore-start -->
{#if a}block1{/if} <script module>
	export const m = 1;
</script><span>inline3</span>
<!-- format-ignore-end -->
<!-- prettier-ignore-start -->
{a} <script>
	let a = 1;
</script>text2
<!-- prettier-ignore-end -->
<!-- prettier-ignore-start -->
text1
<style>
	p {
		color: red;
	}
</style>text2
<!-- prettier-ignore-end -->
<!-- prettier-ignore-start -->
text3

<svelte:options runes />text4
<!-- prettier-ignore-end -->
