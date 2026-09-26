<!--
	A nested `<script>` / `<style>` with a body takes its own line wherever that is render-free: the
	boundary on either side already holds whitespace.
-->

<!-- block element parent -->
<p>text1 <script>let a = 1;</script> text2</p>

<p>text1 <style>div { color: red; }</style> text2</p>

<!-- inline element parent -->
<span>text1 <script>let b = 2;</script> text2</span>

<!-- block body -->
{#if cond}text1 <script>let c = 3;</script> text2{/if}

<!-- component parent -->
<Comp>{expr1} <style>div { color: red; }</style> {expr2}</Comp>
