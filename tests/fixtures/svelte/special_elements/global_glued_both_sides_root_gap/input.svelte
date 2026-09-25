<svelte:options runes />

<script>
	let s = 1;
</script>

<!--
	A <script>, <style> or <svelte:options> between a global `svelte:*` element and its glued
	neighbour is lifted out of the fragment before the whitespace rules run, so the text around it
	is adjacent to the compiler: the element stays glued to it on both sides.
-->

<!-- the script's position -->
text1<svelte:window on:x={fn1} />text2

<!-- the svelte:options position -->
text3<svelte:document on:x={fn2} />text4

<!-- the style's position -->
text5<svelte:body on:x={fn3} />text6

<style>
	p {
		color: red;
	}
</style>
