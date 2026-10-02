<!--
	A nested `<script>` / `<style>` body is raw text: a `<!--` opening it that never closes is
	part of the body, not a comment.
-->

<div><script><!--a</script></div>

<div><style><!-- b</style></div>
