<!-- Test: member expression break on continuation line exceeding print width -->
<!-- BUG: We break the {#if} condition while Prettier tolerates 107 char overage -->
<!-- After breaking first expr, continuation is 107 chars - Prettier keeps it, we break -->
<div>
	<div>
		<div>
			<div>
				<td>
					<a href={u}>
						{aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.bbbb
							.cccc}{#if xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx.y.z}
							&nbsp;{d}
						{/if}
					</a>
				</td>
			</div>
		</div>
	</div>
</div>
