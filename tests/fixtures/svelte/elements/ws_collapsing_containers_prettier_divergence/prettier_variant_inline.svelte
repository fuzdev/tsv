<!--
	Inside a whitespace-collapsing container (select / table / tbody / thead / tfoot / tr /
	colgroup / datalist, and SVG outside a text element) the compiler removes inter-sibling
	whitespace ENTIRELY (clean_nodes can_remove_entirely) — it never renders. So tsv lays these
	containers out block-style: each child on its own line, no rendered space between siblings,
	matching the compiled render. Prettier keeps a container authored inline on one line (see
	prettier_variant_inline.svelte). The div is the control: an ordinary parent's inter-sibling
	space is render-significant, so both formatters keep it inline.
-->
<select><option>a</option> <option>b</option></select>
<table><tbody><tr><td>a</td> <td>b</td></tr> <tr><td>c</td> <td>d</td></tr></tbody></table>
<div><span>a</span> <b>b</b></div>
