<!--
	A frozen nested `<script>` / `<style>` glued to content on both sides keeps both glued
	boundaries: the boundary after it is the one it gets unfrozen, and unfrozen it stays glued.
-->

<div>
	text1<!-- prettier-ignore --><script>let  a  =  1;</script>text2
</div>

{#if cond}
	text3<!-- prettier-ignore --><style>div  {  color:  red;  }</style>text4
{/if}
