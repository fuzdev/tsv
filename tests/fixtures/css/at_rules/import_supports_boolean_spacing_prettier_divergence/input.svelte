<style>
	@import url('a.css') supports((display: grid) and (gap: 1px));
	@import url('b.css') supports((display: grid) or (gap: 1px));
	@import url('c.css') supports(not (display: grid));
</style>
