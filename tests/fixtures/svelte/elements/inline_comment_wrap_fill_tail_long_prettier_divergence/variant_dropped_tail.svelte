<!--
	The fill-content twin of inline_sibling_drop_tail_flow_long: the wrap's other side is a
	comment, but the element's content holds prose and a child, so its width-broken block-style
	form re-parses as the authored-air document — the two are ONE document, and the non-terminal
	tail answers per width from the closing tag's own column in both. Every authoring of each
	case (same-line, dropped-tail) converges on that answer in one pass.
-->

<!-- 100: the tail hugs the intact closing tag exactly at print width -->
<ul>
	<li>
		<!-- c -->
		<a href="/ab" class="class" data-attr1="value1" data-attr2="value2">
			inline1 <code>a</code> inline2
		</a>
		text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11xxxxxxxxxxxxxxx <b>x</b>
	</li>
</ul>

<!-- 101: one wider — the trailing element wraps to its own line -->
<ul>
	<li>
		<!-- c -->
		<a href="/ab" class="class" data-attr1="value1" data-attr2="value2">
			inline1 <code>a</code> inline2
		</a>
		text1 text2 text3 text4 text5 text6 text7 text8 text9 text10 text11xxxxxxxxxxxxxxxx <b>x</b>
	</li>
</ul>
