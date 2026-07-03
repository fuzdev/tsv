<style>
	@import url('a.css') supports((display: grid));
	@import url('b.css') supports(not (display: flex));
	@import url('c.css') supports(selector(a > b));
	@import url('d.css') supports((a: b) and (c: d));
</style>
