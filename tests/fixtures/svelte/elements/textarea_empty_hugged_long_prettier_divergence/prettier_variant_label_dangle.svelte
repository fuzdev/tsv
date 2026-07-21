<!--
	An EMPTY <textarea> hugged by an inline <label>. The whitespace-sensitive element has no
	content, so nothing at the label's boundary is whitespace-significant: the <label> lays out
	block-style like any other inline element, and only the <textarea>'s own attributes react to
	print width. Prettier instead dangles the label's closing delimiter, and the `</label` suffix
	it adds to the textarea line is what pushes the attributes over — see
	prettier_variant_label_dangle.
-->

<!-- 100 chars: the textarea line fits, attributes stay inline -->
<div>
	<div>
		<label>
			<textarea data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
			></textarea></label
		>
	</div>
</div>

<!-- 101 chars: one longer, so the textarea's `>` drops to its own line -->
<div>
	<div>
		<label>
			<textarea data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
			></textarea></label
		>
	</div>
</div>
