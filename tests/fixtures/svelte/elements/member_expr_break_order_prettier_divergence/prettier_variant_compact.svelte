<!-- Member expr + {#if} as inline content: tsv lays it out block-style (both tags intact, content on its own indented line), here unbroken; prettier dangles the tag delimiters. -->
<div>
	<div>
		<div>
			<div>
				<td
					><a href={u}
						>{aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbb
							.cccc}{#if x.y.z}&nbsp;{d}{/if}</a
					></td
				>
			</div>
		</div>
	</div>
</div>
