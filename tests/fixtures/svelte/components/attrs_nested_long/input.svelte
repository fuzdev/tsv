<!-- Component attr wrapping at deep nesting (7 tabs = 14 chars indent) -->
<!-- Line 8 with inline attrs would be 102 chars - exceeds 100, so Prettier wraps -->
<!-- Same attrs at indent level 1 would be 90 chars - no wrap -->
<div>
	<div>
		<div>
			<div>
				<div>
					<div>
						<small class="aaaa"
							>{#if cond}<Comp
									name={prop_value}
									data-attr="var(--aaaa_bb)"
								/>&nbsp;{/if}{expr}</small
						>
					</div>
				</div>
			</div>
		</div>
	</div>
</div>
