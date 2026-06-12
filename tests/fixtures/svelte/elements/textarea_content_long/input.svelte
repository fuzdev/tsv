<!-- Textarea with content - closing > moves to new line at boundary -->

<!-- 100 chars with >: fits on one line -->
<div>
	<div>
		<label>
			<div>text</div>
			<textarea data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">content</textarea>
		</label>
	</div>
</div>

<!-- 101 chars with >: attrs and content stay inline, > on next line -->
<div>
	<div>
		<label>
			<div>text</div>
			<textarea data-attr="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa">content</textarea
			>
		</label>
	</div>
</div>
