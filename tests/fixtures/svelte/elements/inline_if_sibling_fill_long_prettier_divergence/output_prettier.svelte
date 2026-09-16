<!-- A {#if} with an inline <span> sibling before it and a breakable <span> body, at the boundary of the block-head line the dangled </span> closing > opens: at exactly 100 the block stays inline on that fresh line, at 101 tsv drops the body to its own line. Prettier hugs the } at both widths and breaks the inner <span> internally (prettier_variant_inline) -->
<div>
	<div>
		<div>
			<small>
				<span>{obj.prop_name_abc}</span>{#if obj.cond_prop_name_abcdefghij}<span
						>{obj.prop_name_abcdefghijklmnopqrstuv}</span
					>{/if}
			</small>
		</div>
	</div>
</div>

<!-- one char wider: the dangled block-head line overflows too, so the body drops -->
<div>
	<div>
		<div>
			<small>
				<span>{obj.prop_name_abc}</span>{#if obj.cond_prop_name_abcdefghij}
					<span>{obj.prop_name_abcdefghijklmnopqrstuvw}</span>
				{/if}
			</small>
		</div>
	</div>
</div>
