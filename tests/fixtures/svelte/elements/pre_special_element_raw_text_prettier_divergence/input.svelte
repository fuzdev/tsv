<!--
	A nested `<script>` / `<style>` among a special element's content inside `<pre>` keeps its body
	byte-for-byte, like the rest of that content.
-->

<pre>a<svelte:element this="b"><script>let q = 1;</script>text1   text2</svelte:element>c</pre>

<pre>a<svelte:component this={Comp}
		>text1   text2<style>div { color: red; }</style></svelte:component
	>c</pre>
