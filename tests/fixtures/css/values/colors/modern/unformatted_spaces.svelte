<style>
	div {
		/* HWB color space (CSS Color 4) */
		color: hwb(  180  20%  30%  );
		background: hwb(  0  0%  0%  /  50%  );
		/* Lab color space (CSS Color 4) */
		color: lab(  50%  40  60  );
		background: lch(  60%  50  180  );
		/* OKLab color space (CSS Color 4) */
		border-color: oklab(  60%  0.15  -0.12  );
		outline-color: oklch(  60%  0.15  180  );
		/* color() function (CSS Color 4) */
		fill: color(  display-p3  0.5  0.7  0.3  );
		stroke: color(  srgb  0.8  0.2  0.1  );
		/* color-mix() function (CSS Color 5) */
		box-shadow: 0 0 10px color-mix(  in  lch,  red  30%,  blue  );
		text-shadow: 1px 1px color-mix(  in  oklch,  currentColor  50%,  transparent  );
		/* Relative colors (CSS Color 5) */
		--adjusted: oklch(  from  red  calc(l * 1.2)  c  h  );
		--lighter: lab(  from  blue  calc(l + 10)  a  b  );
		/* light-dark() function (CSS Color 5) */
		color: light-dark(  black,  white  );
		background: light-dark(  #fff,  #000  );
		/* contrast-color() function (CSS Color 6) */
		border-color: contrast-color(  blue  );
		outline-color: contrast-color(  var(--bg)  );
	}
</style>
