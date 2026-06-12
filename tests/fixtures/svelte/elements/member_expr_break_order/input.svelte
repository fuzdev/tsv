<!-- Test: member expression break ORDER when multiple expressions exist in inline content -->
<!-- BUG: We break the {#if} condition while Prettier breaks the FIRST expression -->
<!-- Prettier breaks first expr at .bbbb.cccc; we incorrectly break condition at .y.z -->
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
