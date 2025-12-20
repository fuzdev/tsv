<style>
	/* Test dimension values with various units */
	div {
		/* Length units - absolute */
		width: 100px;
		height: 2cm;
		margin: 1mm;
		padding: 72pt;
		border-width: 6pc;
		top: 1in;
		left: 10Q;
		/* Length units - font-relative */
		font-size: 16px;
		line-height: 1.5em;
		letter-spacing: 0.1rem;
		word-spacing: 2ex;
		tab-size: 4ch;
		/* Length units - viewport-relative */
		max-width: 80vw;
		max-height: 90vh;
		min-width: 10vmin;
		min-height: 10vmax;
		/* Modern viewport units (small) */
		width: 100svw;
		height: 100svh;
		min-width: 10svmin;
		max-height: 90svmax;
		/* Modern viewport units (large) */
		width: 100lvw;
		height: 100lvh;
		/* Modern viewport units (dynamic) */
		width: 100dvw;
		height: 100dvh;
		/* Modern viewport units (logical) */
		inline-size: 50vi;
		block-size: 50vb;
		/* Container query units */
		padding: 5cqw 5cqh;
		margin: 2cqi 2cqb;
		gap: 1cqmin;
		flex-basis: 10cqmax;
		/* Typography units (Level 4) */
		font-size: 2cap;
		width: 10ic;
		line-height: 2lh;
		margin-top: 1rlh;
		/* Percentage */
		opacity: 50%;
		/* Angle units */
		transform: rotate(45deg);
		transform: rotate(1rad);
		transform: rotate(0.5turn);
		transform: rotate(100grad);
		/* Time units */
		transition-duration: 300ms;
		animation-duration: 2s;
		/* Frequency units */
		pitch: 440Hz;
		pitch: 1kHz;
		/* Resolution units */
		resolution: 300dpi;
		resolution: 2dppx;
		resolution: 96dpcm;
	}
</style>
