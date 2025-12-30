<style>
	@import url('./a.css') screen and (min-width: 768px);
	@import './b.css' print and (color);
	@import url('./c.css') layer(base) supports(display: grid) screen and (max-width: 1024px);
</style>
