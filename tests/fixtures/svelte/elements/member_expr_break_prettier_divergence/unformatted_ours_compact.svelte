<!-- Member expression as inline content: tsv lays it out block-style (both tags intact, content
     on its own indented line, well within printWidth here so it stays unbroken). The two cases
     pin the boundary in prettier's dangle form: the first dangles the content at exactly 100 chars
     (unbroken), the second at 101 (prettier breaks the member expression at .eeeeee). tsv's
     block-style form is identical-shape and unbroken in both — the divergence is the layout, not
     the expression break. -->

<!-- Dangled inline form is exactly 100 chars (prettier keeps the member expression unbroken) -->
<div><section><table><tbody>{#each items as item}<tr><td><a href={fn(`/${aa.bb.ccccccxyyyy}`)}>{aa.bb.ccccccxyyyy}{#if aa.bb.ddddddxyy.eeeeee}text{aa.bb.ddddddxyy.eeeeee}{/if}</a></td></tr>{/each}</tbody></table></section></div>

<!-- Dangled inline form is 101 chars (prettier breaks the member expression to stay within width) -->
<div><section><table><tbody>{#each items as item}<tr><td><a href={fn(`/${aa.bb.ccccccxyyyyy}`)}>{aa.bb.ccccccxyyyyy}{#if aa.bb.ddddddxyy.eeeeee}text{aa.bb.ddddddxyy.eeeeee}{/if}</a></td></tr>{/each}</tbody></table></section></div>
