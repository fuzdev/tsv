<!-- Two breakable expressions compete for the same line in a fill that overflows. Only one needs
     to break, so the layout has a choice to make — and it must make the same one every pass. -->
<div>
	<div>
		<div>
			<small>{expr1abcdef} text1{expr1abcdef !== 1 ? 's' : ''}, {obj.expr2abcdef} text2{obj.expr2abcdef !== 1 ? 's' : ''}
			</small>
		</div>
	</div>
</div>

<!-- Contrast: the same shape, short enough to fit — neither expression breaks -->
<div>
	<small>{expr1} text1{expr1 !== 1 ? 's' : ''}, {expr2} text2{expr2 !== 1 ? 's' : ''}</small>
</div>
