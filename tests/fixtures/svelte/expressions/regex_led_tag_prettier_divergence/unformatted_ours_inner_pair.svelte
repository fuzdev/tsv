<!-- in text position a tag may not print `{/`, which Svelte reads as a block close -->
{(/a/).test(b)}
<div>{(/a/).test(b)}</div>
<div>text1 {(/a/) ? b : c} text2</div>

<!-- a comment ahead of the literal, or a pair the printer keeps, keeps the brace off it -->
<div>{/* c */ /a/.test(b)}</div>
<div>{(/a/ + b).c}</div>
<div>{(/a/).x = 1}</div>

<!-- an attribute value and a textarea's content read `{/` as an expression -->
<div data-attr={/a/.test(b)}></div>
<textarea>{/a/.test(b)}</textarea>
