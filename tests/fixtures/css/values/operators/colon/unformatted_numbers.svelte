<style>
	div {
		/* a `:` inside a function or a group is its own token: it never takes a
		space before and always takes one after */
		margin: f(a: 1.50);
		padding: f(a: 1.50);
		width: f(a: 1.50);
		height: (a: 1.50);
		top: f(1.50: 2.50);
		right: f(a: 1.50, b: 2.50);
		/* a url() argument is opaque, so its `:` is content */
		bottom: url(a:1.50);
	}
</style>
