<!-- Long inline content with {#if} sibling - tests fill mode break points -->
<!-- At 4-tab indent (8 visual), combined content ~80 chars forces fill wrap -->
<!-- Prettier keeps close tag + {#if} together, breaks inner <span> separately -->
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
