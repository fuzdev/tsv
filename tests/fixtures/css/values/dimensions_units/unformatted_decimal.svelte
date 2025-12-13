<style>
	/* Test dimension values with various units */
	div {
		/* Length units - absolute */
		width: 100.0px;
		height: 2.0cm;
		margin: 1.00mm;
		padding: 72.0pt;
		border-width: 6.0pc;
		top: 1.0in;
		left: 10.0Q;
		/* Length units - font-relative */
		font-size: 16.0px;
		line-height: 1.50em;
		letter-spacing: 0.10rem;
		word-spacing: 2.0ex;
		tab-size: 4.0ch;
		/* Length units - viewport-relative */
		max-width: 80.0vw;
		max-height: 90.0vh;
		min-width: 10.0vmin;
		min-height: 10.0vmax;
		/* Modern viewport units (small) */
		width: 100.0svw;
		height: 100.0svh;
		min-width: 10.0svmin;
		max-height: 90.0svmax;
		/* Modern viewport units (large) */
		width: 100.0lvw;
		height: 100.0lvh;
		/* Modern viewport units (dynamic) */
		width: 100.0dvw;
		height: 100.0dvh;
		/* Modern viewport units (logical) */
		inline-size: 50.0vi;
		block-size: 50.0vb;
		/* Container query units */
		padding: 5.0cqw 5.0cqh;
		margin: 2.0cqi 2.0cqb;
		gap: 1.0cqmin;
		flex-basis: 10.0cqmax;
		/* Typography units (Level 4) */
		font-size: 2.0cap;
		width: 10.0ic;
		line-height: 2.0lh;
		margin-top: 1.0rlh;
		/* Percentage */
		opacity: 50.0%;
		/* Angle units */
		transform: rotate(45.0deg);
		transform: rotate(1.0rad);
		transform: rotate(0.50turn);
		transform: rotate(100.0grad);
		/* Time units */
		transition-duration: 300.0ms;
		animation-duration: 2.0s;
		/* Frequency units */
		pitch: 440.0Hz;
		pitch: 1.0kHz;
		/* Resolution units */
		resolution: 300.0dpi;
		resolution: 2.0dppx;
		resolution: 96.0dpcm;
	}
</style>
