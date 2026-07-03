<style>
	a {
		background: url(foo/*bar);
	}
	b {
		background: url(http://a.fuz.dev/x/*y);
	}
	c {
		background: url(a*/b);
	}
	d {
		background: url(/*x*/);
	}
	e {
		background: url('a/*b');
	}
</style>
