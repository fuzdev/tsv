<!--
	Inside <pre>, an element's wrapped attributes (and its `/>`) indent off the
	element's nesting depth, which accumulates one level per enclosing container
	(element body, block body) exactly like prettier. The columns of the tags come
	from preserved text; only the attribute/`>` breaks are doc-driven.
-->

<!-- Self-closing component directly in <pre>: attrs at 2 tabs, `/>` at 1 -->
<pre><Comp prop={value} onhover={(node)=>{handle(node);}}/></pre>

<!-- Nested through code > if > ul: depth accumulates, attrs at 5 tabs, `/>` at 4 -->
<pre>
	<code>
		{#if cond}
			<ul>
				<Comp prop1={value1} prop2={value2} onhover={(node)=>{if(node===null){handle(node);}}}/>
			</ul>
		{:else}
			<p>text</p>
		{/if}
	</code>
</pre>
