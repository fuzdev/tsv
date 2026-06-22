<!-- Binary ?? with multi-line call as left operand -->
<!-- ?? right side stays on same line as ) when it fits -->

<!-- Short call - stays inline -->
<p>{a.b.fn(x, y) ?? 'text'}</p>

<!-- Call args break but ?? stays on closing ) line -->
<div>
	<div>
		<div>
			<div>
				<div>
					<div>
						<button>
							{a.b.c.fn(
								aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
								bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb + '/'
							) ?? 'text'}
						</button>
					</div>
				</div>
			</div>
		</div>
	</div>
</div>
