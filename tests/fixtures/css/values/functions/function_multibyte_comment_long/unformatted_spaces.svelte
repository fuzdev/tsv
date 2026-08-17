<style>
	div {
		/* Multibyte comment width is visual, not byte length. Exactly 100 cols: stays inline. */
		prop:   fn( /* ééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééé */  x) ;
		/* One col longer (101): the function wraps to multiline args. */
		prop:  fn(  /* éééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééé */  x  ) ;
		/* Wrapped: a multibyte leading comment stays inline; trailing values fill-break. */
		prop:  fn(  /* éééééééééééééééééééééééééééééééééééééééééééééééééé */  aaaa  bbbb  cccc  dddd  eeee  ffff  gggg  hhhh  iiii  jjjj ,  second  ) ;
	}
</style>
