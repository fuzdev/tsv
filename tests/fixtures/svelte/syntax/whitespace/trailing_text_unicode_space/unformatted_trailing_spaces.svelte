<!--
	A document's trailing text is trimmed with Svelte's own class, JS `\s`. U+0085 NEL is
	NOT in it, so it survives as the last Text node and both formatters keep it; the JS
	`\s` members padding it after are dropped (see unformatted_trailing_unicode_space,
	unformatted_trailing_spaces). Rust's `White_Space` is the wrong class in both
	directions here — it has NEL and lacks U+FEFF.
-->
<div>block1</div>
  	  
