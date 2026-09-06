<style>
	/* the escaped `#` ends no token, so the separator after the ident is content */
	@supports (a: x\#FFF 5px) and (b: x\#FFF y) {
		c {
			color: red;
		}
	}
</style>
