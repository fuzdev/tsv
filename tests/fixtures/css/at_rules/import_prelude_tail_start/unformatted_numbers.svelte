<style>
	/* the component after the url()/string is any value, not only a media type or condition:
	a number, a hex colour, a url() and a string all take the value reader's normalization */
	@import 'a.css' 1e3;
	@import 'a.css' 5;
	@import 'a.css' #fff;
	@import 'a.css' url(x);
	@import 'a.css' 'b';
	@import url(a.css) 1px;
	/* the same after a layer() and ahead of a media type or condition */
	@import 'a.css' layer(a) 1e3;
	@import 'a.css' 1e3 screen;
	@import 'a.css' 1e3 and (a: .5px);
	/* the tail ends at a block as it does at a `;` */
	@import 'a.css' screen {
	}
</style>
