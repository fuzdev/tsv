<script lang="ts">
	let a: any;
</script>

<!-- short - stays inline -->
<div data-attr={a.b === a.c || a.b.d(a.c) ? 'value1' : undefined}>text</div>

<!-- binary || breaks in ternary test - continuation at base indent (no extra indent) -->
<div
	data-attr={a.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb === a.ccccccc ||
	a.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.ffffff(a.ccccccc)
		? 'value1'
		: undefined}
>
	text
</div>

<!-- inline element (<a>) with same pattern - text indent matches block elements -->
<a
	data-attr={a.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb === a.ccccccc ||
	a.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.ffffff(a.ccccccc)
		? 'value1'
		: undefined}
>
	text
</a>

<!-- deeply nested - same pattern at higher indent -->
<div>
	<div>
		<div>
			<div>
				<div>
					<div>
						<div>
							<div>
								<div>
									<a
										data-attr1="value1"
										data-attr2={a.bbbbbbbbbbbbbbbbbbb === a.ddddd ||
										a.bbbbbbbbbbbbbbbbbbb.ffffff(a.ddddd)
											? 'value1'
											: undefined}
									>
										text
									</a>
								</div>
							</div>
						</div>
					</div>
				</div>
			</div>
		</div>
	</div>
</div>
