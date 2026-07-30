<style>
	@import url('a.css') supports((display: grid));
	@import url('b.css') supports(not (display: flex));
	@import url('c.css') supports(selector(a > b));
	@import url('d.css') supports((a: b) and (c: d));
	@import url('e.css') supports((width: 0.5px));
	@import url('f.css') supports((color: #fff));
	@import url('g.css') supports(selector([data-attr=1.50]/* c */.class));
</style>
