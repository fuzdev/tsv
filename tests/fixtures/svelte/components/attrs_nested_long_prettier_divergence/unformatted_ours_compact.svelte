<!-- A {#if} body (a component with attrs) exceeds printWidth at 7-tab nesting: tsv drops the body to its own line; prettier hugs the } and wraps the component's attributes (prettier_variant_hug) -->
<div><div><div><div><div><div><small class="aaaa">{#if cond}<Comp name={prop_value} data-attr="var(--aaaa_bb)"/>&nbsp;{/if}{expr}</small></div></div></div></div></div></div>
