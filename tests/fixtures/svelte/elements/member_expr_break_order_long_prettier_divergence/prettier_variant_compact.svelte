<!-- Wide member expr + {#if} as inline content: tsv lays it out block-style and treats printWidth as a hard limit (the member expr wraps at .cccc to stay <=100); prettier dangles the tags and tolerates a 107-char over-width line. -->
<div>
	<div>
		<div>
			<div>
				<td
					><a href={u}
						>{aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbb
							.cccc}{#if xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx.y.z}&nbsp;{d}{/if}</a
					></td
				>
			</div>
		</div>
	</div>
</div>
