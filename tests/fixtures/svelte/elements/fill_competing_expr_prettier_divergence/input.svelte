<!--
	Two breakable expressions share a fill that overflows. The run resolves at the render-free
	whitespace boundary in front of the first welded unit that no longer fits — text2{…} travels
	whole and NEITHER expression breaks — and every authoring of the content boundary (padded,
	hugged, dangled) must converge to that same form instead of oscillating.
-->
<div>
	<div>
		<div>
			<small>
				{expr1abcdef} text1{expr1abcdef !== 1 ? 's' : ''}, {obj.expr2abcdef}
				text2{obj.expr2abcdef !== 1 ? 's' : ''}
			</small>
		</div>
	</div>
</div>

<!-- Contrast: the same shape, short enough to fit — neither expression breaks -->
<div>
	<small>{expr1} text1{expr1 !== 1 ? 's' : ''}, {expr2} text2{expr2 !== 1 ? 's' : ''}</small>
</div>
