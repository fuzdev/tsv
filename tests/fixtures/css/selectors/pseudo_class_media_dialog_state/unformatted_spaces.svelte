<style>
	/* Media playback states */
	video:playing  {
		border :  2px  solid  blue ;
	}

	video:paused  {
		opacity :  0.5 ;
	}

	video:seeking  {
		opacity :  0.8 ;
	}

	/* Media loading states */
	video:buffering::after  {
		content :  'Loading...' ;
	}

	video:stalled  {
		border :  2px  solid  red ;
	}

	/* Audio sound states */
	audio:muted  {
		opacity :  0.5 ;
	}

	audio:volume-locked::before  {
		content :  '[locked]' ;
	}

	/* Display presentation states */
	:fullscreen  {
		background :  black ;
	}

	:picture-in-picture  {
		border :  2px  solid  blue ;
	}

	/* Dialog/modal states */
	dialog:modal  {
		backdrop-filter :  blur( 5px ) ;
	}

	div:popover-open  {
		display :  block ;
	}

	/* Web component states */
	custom-element:defined  {
		opacity :  1 ;
	}

	/* Time-dimensional states (WebVTT) */
	::cue:current  {
		color :  yellow ;
	}

	::cue:past  {
		color :  gray ;
	}

	::cue:future  {
		color :  white ;
	}

	/* Functional form */
	p:current  {
		background :  yellow ;
	}

	p:current( li , dt , dd )  {
		font-weight :  bold ;
	}
</style>
