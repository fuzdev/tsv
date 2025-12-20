<style>
	div {
		/* short linear-gradient - stays inline (under 100 chars) */
		background: linear-gradient(  to  right,  #ff0000,  #00ff00,  #0000ff  );
		/* long linear-gradient - wraps (direction + rgba + stops) */
		background: linear-gradient(  to  bottom  right,  rgba(  255,  255,  255,  0.8  )  0%,  rgba(  0,  0,  0,  0.8  )  50%,  rgba(  128,  128,  128,  0.8  )  100%  );
		/* long radial-gradient - wraps */
		background: radial-gradient(  circle  at  center,  rgba(  255,  0,  0,  0.8  )  0%,  rgba(  0,  255,  0,  0.8  )  50%,  rgba(  0,  0,  255,  0.8  )  100%  );
		/* long conic-gradient - wraps */
		background: conic-gradient(  from  45deg  at  50%  50%,  red  0deg,  yellow  90deg,  lime  180deg,  cyan  270deg,  blue  360deg  );
		/* long repeating-linear-gradient - wraps */
		background: repeating-linear-gradient(  45deg,  #606dbc  0px,  #606dbc  10px,  #465298  10px,  #465298  20px  );
	}
</style>
