<style>
	div {
		/* a value starting with `progid:` is opaque: no number, quote, hex or unit normalization */
		filter:progid:X.Y(a=1.50, b="#FF0000") 1.50PX;
		/* its interior whitespace is kept as written, between members and inside the parens */
		filter:progid:X.Y(a=1.50)    progid:Z.W(b=2);
		filter:progid:X.Y( a = 1.50 , b=#FF0000 ) 1.50px;
		/* a newline inside the value is kept too */
		filter:progid:X.Y(a=1.50)
			progid:Z.W(b=2);
		/* a leading comment run is between-material: it stays on the colon's line and the value
		   after it still freezes */
		filter: /* c */ progid:X.Y(a=1.50);
		/* `!important` is not part of the value and normalizes as usual */
		filter:progid:X.Y(a=1.50) !important;
		/* an opaque value never wraps, however wide (129 columns) */
		filter:progid:DXImageTransform.Microsoft.gradient(startColorstr='#FF0000', endColorstr='#00FF00', GradientType=1.50) 1.50px;
		/* the bare prefix is a value too */
		filter:progid:;
		/* a quoted value is a string, not the trigger: it re-quotes like any other */
		-ms-filter: 'progid:DXImageTransform.Microsoft.Alpha(Opacity=50)';
	}
</style>
