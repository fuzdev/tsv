<!-- A {#if} with an inline <span> sibling before it and a breakable <span> body: when content exceeds printWidth, tsv keeps the </span>{#if} boundary hugged and drops the body to its own line; prettier hugs the } and breaks the inner <span> internally (prettier_variant_hug) -->
<div>
	<div>
		<div>
			<small>
				<span>{obj.prop_name_abc}</span>{#if obj.cond_prop_name_abcdefghij}<span
						>{obj.prop_name_abcdefghij}</span
					>{/if}
			</small>
		</div>
	</div>
</div>
