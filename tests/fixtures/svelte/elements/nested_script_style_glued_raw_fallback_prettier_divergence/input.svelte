<!--
	A nested `<script>` / `<style>` whose body stays as written — a body that fails its parse, or
	one that formats to nothing — glued to content on both sides: both glued boundaries are kept,
	and with no body line to break the element stays inline.
-->

<!-- a style body that fails its parse -->
<p>text1<style>$c: red;</style>text2</p>

<!-- a script body that formats to nothing -->
<p>text1<script>;</script>text2</p>
