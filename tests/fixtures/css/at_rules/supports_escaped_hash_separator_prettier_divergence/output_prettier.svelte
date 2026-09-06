<style>
	/* the escaped `#` ends no token, so the separator after the ident is content */
	@supports (a: x\#FFF5px) and (b: x\#FFFy) {
		c {
			color: red;
		}
	}
</style>
