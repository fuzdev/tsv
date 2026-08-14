<!--
	An attribute's brace interior takes Svelte's `allow_whitespace`, i.e. any JS
	`\s` character, so U+FEFF pads the shorthand and the spread (see
	unformatted_zwnbsp). U+0085 NEL is not in JS `\s` and is rejected there (see
	input_invalid_nel_shorthand).
-->
<div {prop} {...props} data-attr={expr}></div>
